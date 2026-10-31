import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// The live terminals, keyed by the terminal id of each tab's pane; a terminal lives exactly as long as its tab is in the core's state.
@MainActor
@Observable
final class Terminals {
	private(set) var sessions: [UInt64: TerminalSession] = [:]

	/// Drops the terminals of closed tabs, and makes one for each new tab that runs a terminal; `started` gets each new one at once, and its launch, the core's spec, follows.
	///
	/// The terminal exists before its spec has loaded, so that what is typed into a new tab meanwhile waits in it rather than reaching nothing.
	func sync(with tabs: [AppTab], store: Store, started: (AppTab, TerminalSession) -> Void) {
		let live = Set(tabs.map(\.terminalID))
		sessions = sessions.filter { live.contains($0.key) }
		for tab in tabs where sessions[tab.terminalID] == nil && !tab.session.kind.isCreation {
			let id = tab.terminalID
			let session = TerminalSession()
			sessions[id] = session
			started(tab, session)
			Task {
				do {
					let launch = TerminalLaunch(try await store.terminalSpec(id: id))
					// The tab may have closed while its spec loaded.
					if sessions[id] === session { session.start(launch) }
				} catch {
					session.fail(Store.describe(error))
				}
			}
		}
	}

	func session(for tab: AppTab) -> TerminalSession? {
		sessions[tab.terminalID]
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
				TabPane(session: terminals.sessions[tab.terminalID], stack: surfaces.tabStacks[tab.id])
				.opacity(isActive ? 1 : 0)
				.allowsHitTesting(isActive)
			}
		}
		// The tabs' region fills the window even with no tabs, so the window's surfaces, which take its size, do too.
		.frame(maxWidth: .infinity, maxHeight: .infinity)
		.onChange(of: window.tabs, initial: true) { _, tabs in
			surfaces.sync(with: tabs)
			terminals.sync(with: tabs, store: store) { [store, surfaces] tab, session in
				surfaces.pushTerminal(session, tab: tab.id) { store.dispatch(.closeTab(id: tab.id)) }
			}
			surfaces.retireProgressPages(of: tabs)
		}
		.onChange(of: window.activeTab) {
			surfaces.router.focusActiveTab()
		}
	}
}

private struct TabPane: View {
	let session: TerminalSession?
	let stack: LayerStack?

	var body: some View {
		if let reason = session?.startFailure {
			Message(title: "The terminal couldn't start", detail: reason)
		} else if let stack {
			LayerHost(stack: stack)
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
