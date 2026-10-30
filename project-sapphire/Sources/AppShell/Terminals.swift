import CoreBindings
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

	/// Drops the terminals of closed tabs, and starts one for each new tab that runs a terminal, from the core's spec.
	func sync(with tabs: [AppTab], store: Store) {
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
				if slots[id] != nil { slots[id] = slot }
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

	var body: some View {
		let window = store.app.window
		ZStack {
			if window.tabs.isEmpty {
				ContentUnavailableView(
					"No Tabs", systemImage: "terminal",
					description: Text("Open a shell with + or a workspace from the sidebar."))
			}
			ForEach(window.tabs, id: \.terminalID) { tab in
				let isActive = tab.id == window.activeTab
				TabPane(tab: tab, slot: terminals.slots[tab.terminalID], isActive: isActive) {
					store.dispatch(.closeTab(id: tab.id))
				}
				.opacity(isActive ? 1 : 0)
				.allowsHitTesting(isActive)
			}
		}
		.onChange(of: window.tabs, initial: true) { _, tabs in
			terminals.sync(with: tabs, store: store)
		}
	}
}

private struct TabPane: View {
	let tab: AppTab
	let slot: Terminals.Slot?
	let isActive: Bool
	let onExit: () -> Void

	var body: some View {
		switch (tab.session.kind, slot) {
		case (.creatingWorkspace, _):
			ProgressView("Creating the workspace…")
		case (.creationFailed(let reason), _):
			Message(title: "The workspace couldn't be created", detail: reason)
		case (_, .failed(let reason)):
			Message(title: "The terminal couldn't start", detail: reason)
		case (_, .running(let session)):
			TerminalTab(session: session, isActive: isActive, onExit: onExit)
		case (_, .starting), (_, nil):
			Color.clear
		}
	}
}

/// Closes its tab once the terminal's process exits, as Ember does.
private struct TerminalTab: View {
	let session: TerminalSession
	let isActive: Bool
	let onExit: () -> Void

	var body: some View {
		TerminalPane(session: session, isFocused: isActive, router: { _ in false }, onFocus: {})
			.onChange(of: session.exitCode != nil || session.isClosed) { _, ended in
				if ended { onExit() }
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
