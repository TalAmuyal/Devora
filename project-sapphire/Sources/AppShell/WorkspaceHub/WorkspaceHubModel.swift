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

	@ObservationIgnored var openWorkspace: (Workspace) -> Void = { _ in }
	@ObservationIgnored var showCheatsheet: () -> Void = {}
	@ObservationIgnored private let isEditingText: () -> Bool
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private var loadTask: Task<Void, Never>?
	@ObservationIgnored private var refreshTask: Task<Void, Never>?

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

	/// Reloads silently when a page over the hub closes, keeping the filters, the cursor and the statuses already loaded.
	func reload() {
		list(freshStatuses: false) { await $0.store.load() }
	}

	/// Reloads everything with a toast: "Refreshing…", then "Refreshed successfully" unless a call failed, which has its banner; the latest press wins.
	func refresh() {
		refreshTask?.cancel()
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

	func selectProfile(_ path: String) {
		guard path != store.selectedProfilePath else { return }
		cursor.select(0)
		list(freshStatuses: true) { await $0.store.dispatch(.selectProfile(path: path)).value }
	}

	func select(_ workspace: Workspace) {
		if let index = items.firstIndex(where: { $0.path == workspace.path }) { cursor.select(index) }
	}

	/// The layer's cleanup: stops what is still loading, so nothing changes a closed hub.
	func close() {
		loadTask?.cancel()
		refreshTask?.cancel()
	}

	/// The hub's keys; `q` and Escape go on to the stack's dismissal, unless the filter has the focus, where Escape leaves the field and `q` types.
	func handleKey(_ key: Layers.KeyPress) -> Bool {
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

	/// Runs `listing`, then loads the statuses; a later listing wins over an earlier one.
	private func list(freshStatuses: Bool, _ listing: @escaping @MainActor (WorkspaceHubModel) async -> Void) {
		loadTask?.cancel()
		loadTask = Task { [weak self] in
			guard let self else { return }
			await listing(self)
			guard !Task.isCancelled else { return }
			loaded = true
			store.dispatch(.loadWorkspaceStatuses(fresh: freshStatuses))
		}
	}
}

extension Layers.KeyPress {
	/// A plain key a page has no use for: consumed, since the page took the focus from the terminal, so it would reach no view and AppKit would beep.
	/// `q`, Escape and Tab go on, to the stack's dismissal and to the window.
	var isUnusedOnAPage: Bool {
		modifiers.isDisjoint(with: [.control, .option, .command]) && characters != "q"
			&& keyCode != KeyCode.escape && keyCode != KeyCode.tab
	}
}
