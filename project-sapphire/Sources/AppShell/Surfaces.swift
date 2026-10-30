import AppKit
import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// The window's layer stacks and its key router, wired to the store: the composition root of the layer system (`Sources/Layers/CLAUDE.md`).
///
/// It keeps one stack per tab, for exactly as long as the tab is in the core's state, with the tab's terminal as its bottom layer.
@MainActor
@Observable
final class Surfaces {
	@ObservationIgnored private(set) var router: LayerRouter!
	private(set) var tabStacks: [UInt64: LayerStack] = [:]
	@ObservationIgnored private let store: Store

	init(store: Store) {
		self.store = store
		router = LayerRouter { [unowned self] in store.app.window.activeTab.flatMap { tabStacks[$0] } }
		router.shortcuts = AppShortcuts(store: store, windowStack: router.windowStack).handle
	}

	var windowStack: LayerStack { router.windowStack }

	/// Makes a stack for each new tab and tears down the stacks of closed tabs.
	func sync(with tabs: [AppTab]) {
		let live = Set(tabs.map(\.id))
		for (id, stack) in tabStacks where !live.contains(id) {
			stack.clear()
			tabStacks[id] = nil
		}
		for tab in tabs where tabStacks[tab.id] == nil {
			let id = tab.id
			tabStacks[id] = router.makeTabStack { [store] in store.app.window.activeTab == id }
		}
	}

	/// Puts a tab's terminal at the bottom of its stack, once the terminal runs.
	func pushTerminal(_ session: TerminalSession, tab: UInt64, onExit: @escaping () -> Void) {
		guard let stack = tabStacks[tab], stack.isEmpty else { return }
		var spec = terminalLayer(LayerSpec(name: "terminal") { TerminalTab(session: session, onExit: onExit) })
		spec.focus = { session.focus() }
		stack.push(spec)
	}
}

/// Closes its tab once the terminal's process exits, as Ember does.
private struct TerminalTab: View {
	let session: TerminalSession
	let onExit: () -> Void

	var body: some View {
		TerminalPane(session: session)
			.onChange(of: session.exitCode != nil || session.isClosed) { _, ended in
				if ended { onExit() }
			}
	}
}

/// Hands the window to the key router once the view is in it, so that the router sees the window's key-downs.
struct KeyRouterInstaller: NSViewRepresentable {
	let router: LayerRouter

	func makeNSView(context: Context) -> InstallerView {
		InstallerView(router: router)
	}

	func updateNSView(_ view: InstallerView, context: Context) {}

	final class InstallerView: NSView {
		private let router: LayerRouter

		init(router: LayerRouter) {
			self.router = router
			super.init(frame: .zero)
		}

		@available(*, unavailable)
		required init?(coder: NSCoder) { fatalError() }

		override func viewDidMoveToWindow() {
			super.viewDidMoveToWindow()
			if let window { router.install(in: window) } else { router.uninstall() }
		}
	}
}
