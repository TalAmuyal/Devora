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
	/// The open Workspace Hub's model; nil while it is closed.
	@ObservationIgnored private(set) var hub: WorkspaceHubModel?
	/// The open cheatsheet's scrolling; nil while it is closed.
	@ObservationIgnored private(set) var cheatsheet: VimScroll?
	@ObservationIgnored private let store: Store

	init(store: Store) {
		self.store = store
		router = LayerRouter { [unowned self] in store.app.window.activeTab.flatMap { tabStacks[$0] } }
		router.shortcuts = AppShortcuts(store: store, windowStack: router.windowStack) { [unowned self] in
			openWorkspaceHub()
		}.handle
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

	/// Opens the Workspace Hub over the window and loads it; it is a page, so a page closing over it reloads it.
	func openWorkspaceHub() {
		guard windowStack.find(WorkspaceHubModel.layerName) == nil else { return }
		let router = router!
		let model = WorkspaceHubModel(
			store: store, isEditingText: { router.isEditingText }, clearFocus: { router.clearFocus() })
		var spec = pageLayer(LayerSpec(name: WorkspaceHubModel.layerName) { WorkspaceHubView(model: model) })
		spec.onKey = model.handleKey
		spec.onReveal = model.reload
		spec.onCleanup = { [unowned self] in
			model.close()
			if hub === model { hub = nil }
		}
		let handle = windowStack.push(spec)
		// Opening a workspace closes the hub without asking it, as Ember's `dismissWsHub` does.
		model.openWorkspace = { [unowned self, store] workspace in
			windowStack.remove(handle)
			store.dispatch(.openWorkspace(workspace))
		}
		model.showCheatsheet = { [unowned self] in openCheatsheet() }
		hub = model
		model.load()
	}

	/// The hub's shortcuts on a page of their own; `?` closes it as it opened it, and the hub's keys reach nothing meanwhile.
	private func openCheatsheet() {
		guard windowStack.find(Cheatsheet.layerName) == nil else { return }
		let scroll = VimScroll()
		var spec = pageLayer(LayerSpec(name: Cheatsheet.layerName) { Cheatsheet(scroll: scroll) })
		var handle: LayerHandle?
		spec.onKey = { [unowned self] key in
			if key.characters == "?", key.modifiers.isDisjoint(with: [.control, .option, .command]) {
				if let handle { windowStack.remove(handle) }
				return true
			}
			return scroll.handle(key) || key.isUnusedOnAPage
		}
		spec.onCleanup = { [unowned self] in
			if cheatsheet === scroll { cheatsheet = nil }
		}
		handle = windowStack.push(spec)
		cheatsheet = scroll
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
