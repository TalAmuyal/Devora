import CoreBindings
import Layers
import Store
import SwiftUI

/// One opening of the Agent Hub (ADR-012): the selected profile's Agent Spaces, with the Workspace Hub's keyboard model.
@MainActor
@Observable
final class AgentHubModel {
	static let layerName = "agent-hub"

	let store: Store
	private(set) var cursor = ListCursor()
	/// False until the first listing is done, while the list shows its loading placeholder.
	private(set) var loaded = false

	@ObservationIgnored var open: (AgentSpace) -> Void = { _ in }
	@ObservationIgnored var newAgent: () -> Void = {}
	/// Switches to the Workspace Hub, as `Ctrl+S` does.
	@ObservationIgnored var switchHub: () -> Void = {}
	/// Asks with a confirmation dialog, then runs the action if the user confirms.
	@ObservationIgnored var confirm: (Confirmation, @escaping () -> Void) -> Void = { _, _ in }
	@ObservationIgnored private var loadTask: Task<Void, Never>?

	init(store: Store) {
		self.store = store
	}

	var items: [AgentSpace] { store.agentSpaces }

	var focused: AgentSpace? {
		let items = items
		return cursor.position(in: items.count).map { items[$0] }
	}

	var profileName: String? {
		store.profiles.first { $0.path == store.selectedProfilePath }?.name
	}

	/// Loads the profiles, then lists the selected profile's spaces; a later listing wins over an earlier one.
	/// The workspaces are the Workspace Hub's to list, so passing through this hub on the way there lists them once.
	func load() {
		loadTask?.cancel()
		loadTask = Task { [weak self] in
			guard let self else { return }
			_ = await store.loadProfiles()
			guard !Task.isCancelled else { return }
			await store.listAgentSpaces()
			guard !Task.isCancelled else { return }
			loaded = true
		}
	}

	func select(_ space: AgentSpace) {
		if let index = items.firstIndex(of: space) { cursor.select(index) }
	}

	/// An invalid space (an interrupted creation, an unreadable database) can only be deleted.
	func openFocused() {
		guard let focused, focused.error == nil else { return }
		open(focused)
	}

	/// Deleting removes the space's worktrees, its memory and its conversation, so it always asks; an open agent's tab closes first.
	func delete(_ space: AgentSpace) {
		let confirmation = Confirmation(
			title: "Delete agent “\(space.name)”?",
			intro: "Its directory is deleted: \(space.path)",
			warning: "Its worktrees, its memory and its conversation can't be recovered. If it is open, its tab closes first.",
			confirmLabel: "Delete")
		confirm(confirmation) { [weak self] in
			guard let self else { return }
			Task { _ = await self.store.deleteAgentSpace(space.name) }
		}
	}

	func close() {
		loadTask?.cancel()
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.characters {
		case "n": if store.selectedProfilePath != nil { newAgent() }
		case "d": if let focused { delete(focused) }
		case "j": cursor.move(by: 1, in: items.count)
		case "k": cursor.move(by: -1, in: items.count)
		case "R": load()
		default:
			switch key.keyCode {
			case KeyCode.downArrow: cursor.move(by: 1, in: items.count)
			case KeyCode.upArrow: cursor.move(by: -1, in: items.count)
			case KeyCode.returnKey, KeyCode.keypadEnter: openFocused()
			default: return key.isUnusedOnAPage
			}
		}
		return true
	}
}
