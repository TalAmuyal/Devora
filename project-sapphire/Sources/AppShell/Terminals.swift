import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// The live terminals, keyed by the terminal id of each tab's pane; a terminal lives exactly as long as its tab is in the core's state.
@MainActor
@Observable
final class Terminals {
	enum Slot {
		case starting
		case failed(String)
		case running(TerminalSession)
	}

	private(set) var slots: [UInt64: Slot] = [:]

	/// Drops the terminals of closed tabs, and starts one for each new tab that runs a terminal, from the core's spec; `started` gets each one that runs.
	func sync(with tabs: [AppTab], store: Store, started: @escaping (AppTab, TerminalSession) -> Void) {
		let live = Set(tabs.map(\.terminalID))
		slots = slots.filter { live.contains($0.key) }
		for tab in tabs where slots[tab.terminalID] == nil && tab.session.kind.runsTerminal {
			let id = tab.terminalID
			slots[id] = .starting
			Task {
				let slot: Slot
				do {
					slot = .running(TerminalSession(launch: TerminalLaunch(try await store.terminalSpec(id: id))))
				} catch {
					slot = .failed(Store.describe(error))
				}
				// The tab may have closed while its spec loaded.
				guard slots[id] != nil else { return }
				slots[id] = slot
				if case .running(let session) = slot { started(tab, session) }
			}
		}
	}

	func session(for tab: AppTab) -> TerminalSession? {
		guard case .running(let session) = slots[tab.terminalID] else { return nil }
		return session
	}
}

extension SessionKind {
	var runsTerminal: Bool {
		switch self {
		case .shell, .workspace: true
		case .creatingWorkspace, .creationFailed: false
		}
	}
}

extension TerminalLaunch {
	init(_ spec: TerminalSpec) {
		self.init(program: spec.program, args: spec.args, cwd: spec.cwd, env: spec.env, envRemove: spec.envRemove)
	}
}

/// Every tab's content, stacked so that switching tabs keeps each terminal alive; only the active one is visible.
struct TabContent: View {
	let store: Store
	let terminals: Terminals
	let surfaces: Surfaces

	var body: some View {
		let window = store.app.window
		ZStack {
			if window.tabs.isEmpty {
				ContentUnavailableView(
					"No Tabs", systemImage: "terminal",
					description: Text("Open a shell with + or a workspace from the Workspace Hub (Ctrl+S)."))
			}
			ForEach(window.tabs, id: \.terminalID) { tab in
				let isActive = tab.id == window.activeTab
				TabPane(tab: tab, slot: terminals.slots[tab.terminalID], stack: surfaces.tabStacks[tab.id])
				.opacity(isActive ? 1 : 0)
				.allowsHitTesting(isActive)
			}
		}
		.onChange(of: window.tabs, initial: true) { _, tabs in
			surfaces.sync(with: tabs)
			terminals.sync(with: tabs, store: store) { [store, surfaces] tab, session in
				surfaces.pushTerminal(session, tab: tab.id) { store.dispatch(.closeTab(id: tab.id)) }
			}
		}
		.onChange(of: window.activeTab) {
			surfaces.router.focusActiveTab()
		}
	}
}

private struct TabPane: View {
	let tab: AppTab
	let slot: Terminals.Slot?
	let stack: LayerStack?

	var body: some View {
		switch (tab.session.kind, slot) {
		case (.creatingWorkspace, _):
			ProgressView("Creating the workspace…")
		case (.creationFailed(let reason), _):
			Message(title: "The workspace couldn't be created", detail: reason)
		case (_, .failed(let reason)):
			Message(title: "The terminal couldn't start", detail: reason)
		case (_, .running):
			if let stack { LayerHost(stack: stack) }
		case (_, .starting), (_, nil):
			Color.clear
		}
	}
}

private struct Message: View {
	let title: String
	let detail: String

	var body: some View {
		ContentUnavailableView {
			Label(title, systemImage: "exclamationmark.triangle")
		} description: {
			Text(detail).textSelection(.enabled)
		}
	}
}
