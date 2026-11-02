import CoreBindings
import Layers
import Store
import SwiftUI

/// A row of the Settings Hub's master list: the User Defaults (pinned first), a profile, or the New Profile form (pinned last).
enum SettingsRow: Equatable {
	case userDefaults
	case profile(Profile)
	case newProfile

	/// What the detail panel shows for the row: the same id keeps its scope, its form and its drafts.
	var id: String {
		switch self {
		case .userDefaults: "user-defaults"
		case .profile(let profile): "profile:\(profile.path)"
		case .newProfile: "new-profile"
		}
	}
}

/// The counts and repos of a profile's detail, loaded when its row is first focused.
struct ProfileDetail: Equatable {
	var repos: [RepoInfo]
	var workspaceCount: Int
	var activeTaskCount: Int
}

/// One opening of the Settings Hub, Ember's `SettingsHub`: the profiles beside the focused row's detail, on a page layer over the Workspace Hub (`Surfaces.openSettingsHub`).
///
/// The profiles are the store's; it lists them again on opening and after a change it makes (a clone, a deletion).
/// The detail of the User Defaults and of a profile has the settings cards at that scope (`ConfigSettingsModel`); the New Profile row's is the profile form.
@MainActor
@Observable
final class SettingsHubModel {
	static let layerName = "settings-hub"

	/// Which row a new opening focuses: the active profile's, or the New Profile row (the profile dropdown's "New Profile…").
	enum Opening {
		case list
		case newProfile
	}

	let store: Store
	/// False until the profiles are listed, while the list shows its loading placeholder.
	private(set) var loaded = false
	private(set) var cursor = ListCursor()
	/// The loaded details, by profile path.
	private(set) var details: [String: ProfileDetail] = [:]
	/// The focused row's settings, for the User Defaults or a profile.
	private(set) var scope: ConfigSettingsModel?
	/// The New Profile row's form, while that row is focused.
	private(set) var form: ProfileFormModel?
	/// The row `scope` and `form` were made for.
	@ObservationIgnored private var detailRow: String?

	/// Closes the Settings Hub, revealing the Workspace Hub beneath, which reloads.
	@ObservationIgnored var close: () -> Void = {}
	/// Asks with a confirmation dialog, then runs the action if the user confirms; a notice (no Cancel) runs nothing.
	@ObservationIgnored var confirm: (Confirmation, @escaping () -> Void) -> Void = { _, _ in }
	/// Opens the Clone Repo dialog for a profile; the closure runs once the repo was cloned.
	@ObservationIgnored var cloneRepo: (_ profilePath: String, _ onCloned: @escaping () -> Void) -> Void = { _, _ in }
	@ObservationIgnored private let opening: Opening
	@ObservationIgnored private let isEditingText: () -> Bool
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private var detailLoads: Set<String> = []

	init(store: Store, opening: Opening, isEditingText: @escaping () -> Bool, clearFocus: @escaping () -> Void) {
		self.store = store
		self.opening = opening
		self.isEditingText = isEditingText
		self.clearFocus = clearFocus
	}

	var rows: [SettingsRow] {
		[.userDefaults] + store.profiles.map(SettingsRow.profile) + [.newProfile]
	}

	var focusedRow: SettingsRow {
		let rows = rows
		return rows[cursor.position(in: rows.count) ?? 0]
	}

	func isActive(_ profile: Profile) -> Bool { profile.path == store.selectedProfilePath }

	/// Lists the profiles, then focuses the active profile's row (the User Defaults' without one), or the New Profile row.
	func load() {
		Task {
			await store.listProfiles()
			loaded = true
			switch opening {
			case .newProfile:
				cursor.select(rows.count - 1)
			case .list:
				cursor.select(activeRowIndex)
			}
			focusChanged()
		}
	}

	func select(_ row: SettingsRow) {
		guard let index = rows.firstIndex(of: row) else { return }
		cursor.select(index)
		focusChanged()
	}

	/// Makes the profile the active one and closes, as Ember's Set Active and Enter do; the Workspace Hub then lists its workspaces.
	func setActive(_ profile: Profile) {
		store.activateProfile(profile.path)
		close()
	}

	/// Deletes a profile from the registry after a confirmation, or, while open tabs use it, says so in a notice instead.
	/// Deleting the last one closes the Settings Hub over the first-run welcome.
	func delete(_ profile: Profile) {
		let blockers = store.tabs(usingProfile: profile.path)
		guard blockers.isEmpty else {
			confirm(
				Confirmation(
					title: "Cannot delete profile \"\(profile.name)\"",
					intro: "These open sessions are using it:",
					items: blockers.map { .init(name: $0.title, detail: "") },
					warning: "Close them first, then delete the profile.",
					confirmLabel: "OK", cancelLabel: nil, isDestructive: false),
				{})
			return
		}
		confirm(
			Confirmation(
				title: "Delete profile \"\(profile.name)\"?",
				intro: "This removes the profile from Devora's registry only. The directory \(profile.path) and everything in it remains on disk.",
				confirmLabel: "Delete Profile")
		) { [weak self] in
			Task { await self?.performDelete(profile) }
		}
	}

	func cloneRepo(into profile: Profile) {
		cloneRepo(profile.path) { [weak self] in
			Task { await self?.refreshAfterClone(profile.path) }
		}
	}

	/// The page's keys, Ember's: `j`/`k` and the arrows move, Enter sets the focused profile active (or focuses the form), `n` goes to the New Profile form and `d` deletes.
	/// Keys typed into a field are the field's, Escape leaving it; the form's keys are its own while it has the focus; `q` and Escape go on to the stack's dismissal.
	func handleKey(_ key: Layers.KeyPress) -> Bool {
		if let form, form.focus != nil { return handleFormKey(key, form: form) }
		if isEditingText() {
			guard key.keyCode == KeyCode.escape else { return false }
			scope?.focusedKey = nil
			clearFocus()
			return true
		}
		guard loaded, key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.characters {
		case "j": move(by: 1)
		case "k": move(by: -1)
		case "n": focusForm()
		case "d":
			if case .profile(let profile) = focusedRow { delete(profile) }
		default:
			switch key.keyCode {
			case KeyCode.downArrow: move(by: 1)
			case KeyCode.upArrow: move(by: -1)
			case KeyCode.returnKey, KeyCode.keypadEnter:
				switch focusedRow {
				case .profile(let profile): setActive(profile)
				case .newProfile: focusForm()
				case .userDefaults: break
				}
			default: return key.isUnusedOnAPage
			}
		}
		return true
	}

	private func handleFormKey(_ key: Layers.KeyPress, form: ProfileFormModel) -> Bool {
		if key.keyCode == KeyCode.tab, key.modifiers.isSubset(of: .shift) {
			form.cycleFocus(backward: key.modifiers.contains(.shift))
			return true
		}
		if key.keyCode == KeyCode.escape, key.modifiers.isDisjoint(with: [.control, .option, .command]) {
			form.focus = nil
			return true
		}
		return form.handleKey(key)
	}

	private func move(by delta: Int) {
		cursor.move(by: delta, in: rows.count)
		focusChanged()
	}

	/// Focuses the New Profile row and gives its name field the focus on the main actor's next turn: a field made while AppKit dispatches the key that showed it (`n`) never gets the focus it is given.
	private func focusForm() {
		select(.newProfile)
		Task { [weak self] in self?.form?.focus = .name }
	}

	/// Makes the focused row's detail when the row changed, and loads a profile's counts the first time its row is focused.
	private func focusChanged() {
		let row = focusedRow
		guard row.id != detailRow else { return }
		detailRow = row.id
		form = nil
		scope = nil
		clearFocus()
		switch row {
		case .userDefaults:
			showScope(profilePath: nil)
		case .profile(let profile):
			showScope(profilePath: profile.path)
			loadDetail(of: profile.path)
		case .newProfile:
			form = ProfileFormModel(
				store: store, hasCancel: true,
				onCancel: { [weak self] in self?.cancelForm() },
				clearFocus: clearFocus,
				onRegistered: { [weak self] path in
					self?.store.activateProfile(path)
					self?.close()
				})
		}
	}

	private func showScope(profilePath: String?) {
		let scope = ConfigSettingsModel(store: store, profilePath: profilePath)
		self.scope = scope
		Task { await scope.reload() }
	}

	/// The form's Cancel goes back to the active profile's row, as Ember's does.
	private func cancelForm() {
		cursor.select(activeRowIndex)
		focusChanged()
	}

	/// The active profile's row, or the User Defaults' without one.
	private var activeRowIndex: Int {
		store.profiles.firstIndex { $0.path == store.selectedProfilePath }.map { $0 + 1 } ?? 0
	}

	private func loadDetail(of path: String) {
		guard details[path] == nil, !detailLoads.contains(path) else { return }
		detailLoads.insert(path)
		Task {
			defer { detailLoads.remove(path) }
			async let repos = store.registeredRepos(inProfile: path)
			async let workspaces = store.workspaces(inProfile: path)
			// A failed call has its banner; the detail stays loading, as Ember's does.
			guard let repos = await repos, let workspaces = await workspaces else { return }
			details[path] = ProfileDetail(
				repos: repos, workspaceCount: workspaces.count, activeTaskCount: workspaces.filter(\.active).count)
		}
	}

	private func performDelete(_ profile: Profile) async {
		guard await store.unregisterProfile(path: profile.path) else { return }
		details[profile.path] = nil
		if store.profiles.isEmpty {
			// Back to the Workspace Hub, which shows the first-run welcome.
			close()
			return
		}
		focusChanged()
	}

	/// After a clone: the repo counts and the profile's repos are stale.
	private func refreshAfterClone(_ path: String) async {
		details[path] = nil
		await store.listProfiles()
		loadDetail(of: path)
	}
}
