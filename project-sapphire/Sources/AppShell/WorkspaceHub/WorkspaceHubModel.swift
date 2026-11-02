import CoreBindings
import Layers
import Store
import SwiftUI

/// One opening of the Workspace Hub: its filters, its cursor and its toast, over the store's profiles, workspaces and statuses.
///
/// It lives as long as the hub's layer, so closing the hub forgets the filters, as Ember's `unload` does.
@MainActor
@Observable
final class WorkspaceHubModel {
	static let layerName = "workspace-hub"

	/// "Refreshing…" stays up at least this long, even when the reload is instant; "Refreshed successfully" stays this long.
	static let refreshingMinimum = Duration.seconds(1)
	static let refreshedDuration = Duration.seconds(3)
	/// The tooltip of the controls that lead to what Sapphire doesn't have yet, which are shown disabled.
	static let notYet = "Not in Sapphire yet"

	let store: Store
	var category = HubCategory.active {
		didSet { cursor.select(0) }
	}
	var filter = "" {
		didSet { cursor.filterChanged(count: items.count) }
	}
	/// Whether the filter field should have the keyboard focus; the view keeps it in step with the field's focus.
	var filterFocused = false
	private(set) var cursor = ListCursor()
	/// False until the first load is done, while the list shows its loading placeholder.
	private(set) var loaded = false
	private(set) var toast: String?
	/// The header's open dropdown; its popup layer (`Surfaces`) keeps this in step.
	var openMenu: HubMenu?
	/// Where each dropdown's trigger is drawn, in the hub's coordinates, to anchor the open panel under it.
	var menuTriggers: [HubMenu: CGRect] = [:]
	/// The first-run welcome's profile form, while the latest listing found no profiles; the hub can't be closed meanwhile, since there is nothing behind it.
	private(set) var welcome: ProfileFormModel?

	@ObservationIgnored var openWorkspace: (Workspace) -> Void = { _ in }
	@ObservationIgnored var showCheatsheet: () -> Void = {}
	/// Opens a header dropdown, closing the other one; closes it if it is the one open.
	@ObservationIgnored var toggleMenu: (HubMenu) -> Void = { _ in }
	@ObservationIgnored var closeMenu: () -> Void = {}
	/// Asks with a confirmation dialog, then runs the action if the user confirms.
	@ObservationIgnored var confirm: (Confirmation, @escaping () -> Void) -> Void = { _, _ in }
	/// Opens the New Task dialog; with a workspace, it duplicates that workspace's task.
	@ObservationIgnored var newTask: (_ duplicating: Workspace?) -> Void = { _ in }
	/// Opens the Settings Hub, on the active profile or on the New Profile form.
	@ObservationIgnored var openSettings: (SettingsHubModel.Opening) -> Void = { _ in }
	/// Opens the Add Repo dialog for an active workspace.
	@ObservationIgnored var addRepo: (Workspace) -> Void = { _ in }
	@ObservationIgnored private let isEditingText: () -> Bool
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private var loadTask: Task<Void, Never>?
	@ObservationIgnored private var refreshTask: Task<Void, Never>?
	@ObservationIgnored private var toastTask: Task<Void, Never>?
	/// The profile whose workspaces the latest listing listed: another one starts the cursor over.
	@ObservationIgnored private var listedProfile: String?

	init(store: Store, isEditingText: @escaping () -> Bool, clearFocus: @escaping () -> Void) {
		self.store = store
		self.isEditingText = isEditingText
		self.clearFocus = clearFocus
	}

	var items: [Workspace] { store.workspaces.filtered(category: category, filter: filter) }

	var focused: Workspace? {
		let items = items
		return cursor.position(in: items.count).map { items[$0] }
	}

	func status(of workspace: Workspace) -> WorkspaceStatus? {
		store.workspaceStatuses[workspace.path]
	}

	/// Loads the profiles and the workspaces, then their statuses, from scratch.
	func load() {
		loaded = false
		list(freshStatuses: true) { await $0.store.load() }
	}

	/// Reloads silently when a page over the hub closes, keeping the filters, the cursor and the statuses already loaded; after the Settings Hub made another profile active, it loads that one from scratch.
	func reload() {
		if store.selectedProfilePath != listedProfile { loaded = false }
		list(freshStatuses: false) { await $0.store.load() }
	}

	/// Reloads everything with a toast: "Refreshing…", then "Refreshed successfully" unless a call failed, which has its banner; the latest press wins.
	func refresh() {
		refreshTask?.cancel()
		toastTask?.cancel()
		refreshTask = Task { [weak self] in
			guard let self else { return }
			toast = "Refreshing…"
			let started = ContinuousClock.now
			let succeeded = await store.load()
			guard !Task.isCancelled else { return }
			store.dispatch(.loadWorkspaceStatuses(fresh: true))
			try? await Task.sleep(until: started + Self.refreshingMinimum)
			guard !Task.isCancelled else { return }
			toast = nil
			guard succeeded else { return }
			toast = "Refreshed successfully"
			try? await Task.sleep(for: Self.refreshedDuration)
			guard !Task.isCancelled else { return }
			toast = nil
		}
	}

	/// The answer to `q` and Escape: the first-run welcome refuses them, as Ember's zero-profile lock does.
	func dismissDecision() -> DismissDecision {
		welcome == nil ? .close : .veto
	}

	/// After a profile was registered: it is selected, and the hub loads it from scratch.
	func profileRegistered(_ path: String) {
		cursor.select(0)
		loaded = false
		list(freshStatuses: true) { await $0.store.load(selecting: path) }
	}

	func selectProfile(_ path: String) {
		guard path != store.selectedProfilePath else { return }
		cursor.select(0)
		list(freshStatuses: true) { await $0.store.dispatch(.selectProfile(path: path)).value }
	}

	func select(_ workspace: Workspace) {
		if let index = items.firstIndex(where: { $0.path == workspace.path }) { cursor.select(index) }
	}

	/// Ends an active workspace's task: at once when its repos are clean and detached, otherwise after a confirmation listing what will be lost.
	func removeTask(_ workspace: Workspace) {
		switch RemoveTaskDecision(status: status(of: workspace)) {
		case .removeNow:
			performRemoveTask(workspace)
		case .confirm(let problems):
			confirm(RemoveTaskDecision.confirmation(for: workspace, problems: problems)) { [weak self] in
				self?.performRemoveTask(workspace)
			}
		}
	}

	/// After a repo was added to a workspace: the store has listed the workspaces again and dropped that workspace's status, which loads again.
	func repoAdded() {
		store.dispatch(.loadWorkspaceStatuses(fresh: false))
	}

	/// Deletes an inactive or invalid workspace, with no confirmation, as Ember does.
	func deleteWorkspace(_ workspace: Workspace) {
		Task {
			guard await store.deleteWorkspace(workspace) else { return }
			store.dispatch(.loadWorkspaceStatuses(fresh: false))
		}
	}

	/// Writes the latest load's latencies to the profile's `diagnostics/`, and says where in a toast.
	func saveLoadingLatencies() {
		Task {
			guard let path = await store.saveLoadingLatencies() else { return }
			flash("Loading latencies saved: \(path)")
		}
	}

	/// The rows of a header dropdown; the actions that lead to surfaces Sapphire doesn't have yet are shown disabled.
	func menuItems(_ menu: HubMenu) -> [DropdownItem] {
		let notYet = Self.notYet
		switch menu {
		case .profiles:
			return store.profiles.map { profile in
				.option(
					profile.name, detail: profile.repoCount == 1 ? "1 repo" : "\(profile.repoCount) repos",
					checked: profile.path == store.selectedProfilePath
				) { [weak self] in self?.selectProfile(profile.path) }
			} + [
				.separator,
				.action("New Profile…", systemImage: "plus") { [weak self] in self?.openSettings(.newProfile) },
				.action("Manage Profiles…", systemImage: "gearshape") { [weak self] in self?.openSettings(.list) },
			]
		case .main:
			var items: [DropdownItem] = []
			if !store.profiles.isEmpty {
				items.append(.action("Settings", systemImage: "gearshape") { [weak self] in self?.openSettings(.list) })
			}
			items.append(.action("Health", systemImage: "cross.case", enabled: false, help: notYet))
			if store.loadingLatencies != nil {
				items += [.separator, .action("Save loading latencies", systemImage: "square.and.arrow.down") { [weak self] in self?.saveLoadingLatencies() }]
			}
			return items
		}
	}

	/// The layer's cleanup: stops what is still loading, so nothing changes a closed hub.
	func close() {
		loadTask?.cancel()
		refreshTask?.cancel()
		toastTask?.cancel()
	}

	/// The hub's keys; `q` and Escape go on to the stack's dismissal, unless the filter has the focus, where Escape leaves the field and `q` types.
	/// A key that reaches the hub closes an open dropdown first: the dropdown's popup layer passes on what it doesn't claim, and closes on `q` and Escape itself.
	func handleKey(_ key: Layers.KeyPress) -> Bool {
		if openMenu != nil { closeMenu() }
		if let welcome { return handleWelcomeKey(key, form: welcome) }
		if isEditingText() {
			guard key.keyCode == KeyCode.escape else { return false }
			filterFocused = false
			clearFocus()
			return true
		}
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.characters {
		case "?": showCheatsheet()
		case "f": filterFocused = true
		case "1": category = .active
		case "2": category = .inactive
		case "3": category = .all
		case "n": newTask(nil)
		case "P": openSettings(.list)
		case "j": cursor.move(by: 1, in: items.count)
		case "k": cursor.move(by: -1, in: items.count)
		case "R": refresh()
		default:
			switch key.keyCode {
			case KeyCode.downArrow: cursor.move(by: 1, in: items.count)
			case KeyCode.upArrow: cursor.move(by: -1, in: items.count)
			case KeyCode.returnKey, KeyCode.keypadEnter: if let focused { openWorkspace(focused) }
			default: return key.isUnusedOnAPage
			}
		}
		return true
	}

	/// Only the profile form is interactive on the first-run welcome: Tab and Shift+Tab move through it, Escape leaves a field, and the hub's own keys do nothing.
	private func handleWelcomeKey(_ key: Layers.KeyPress, form: ProfileFormModel) -> Bool {
		if key.keyCode == KeyCode.tab, key.modifiers.isSubset(of: .shift) {
			form.cycleFocus(backward: key.modifiers.contains(.shift))
			return true
		}
		if key.keyCode == KeyCode.escape, form.focus?.isTextField == true {
			form.focus = nil
			return true
		}
		return form.handleKey(key)
	}

	/// Shows the first-run welcome when the listing found no profiles, keeping the form typed into while there are still none.
	private func syncWelcome() {
		guard store.profilesListed, store.profiles.isEmpty else {
			if welcome != nil {
				welcome = nil
				clearFocus()
			}
			return
		}
		guard welcome == nil else { return }
		welcome = ProfileFormModel(
			store: store, initialPath: "~/devora", hasCancel: false, clearFocus: clearFocus,
			onRegistered: { [weak self] path in self?.profileRegistered(path) })
	}

	private func performRemoveTask(_ workspace: Workspace) {
		Task {
			guard await store.removeTask(workspace) else { return }
			store.dispatch(.loadWorkspaceStatuses(fresh: false))
		}
	}

	/// Shows a toast for a few seconds, replacing the one up.
	private func flash(_ text: String) {
		toastTask?.cancel()
		toast = text
		toastTask = Task { [weak self] in
			try? await Task.sleep(for: Self.refreshedDuration)
			guard !Task.isCancelled else { return }
			self?.toast = nil
		}
	}

	/// Runs `listing`, then loads the statuses; a later listing wins over an earlier one.
	private func list(freshStatuses: Bool, _ listing: @escaping @MainActor (WorkspaceHubModel) async -> Void) {
		loadTask?.cancel()
		loadTask = Task { [weak self] in
			guard let self else { return }
			await listing(self)
			guard !Task.isCancelled else { return }
			if store.selectedProfilePath != listedProfile {
				listedProfile = store.selectedProfilePath
				cursor.select(0)
			}
			syncWelcome()
			loaded = true
			store.dispatch(.loadWorkspaceStatuses(fresh: freshStatuses))
		}
	}
}

/// The Workspace Hub's header dropdowns: the profiles, and the burger (☰) menu.
enum HubMenu: String {
	case profiles
	case main
}

extension Layers.KeyPress {
	/// A plain key a page has no use for: consumed, since the page took the focus from the terminal, so it would reach no view and AppKit would beep.
	/// `q`, Escape and Tab go on, to the stack's dismissal and to the window.
	var isUnusedOnAPage: Bool {
		modifiers.isDisjoint(with: [.control, .option, .command]) && characters != "q"
			&& keyCode != KeyCode.escape && keyCode != KeyCode.tab
	}
}
