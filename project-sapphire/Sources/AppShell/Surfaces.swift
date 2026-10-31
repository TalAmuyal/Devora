import AppKit
import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// The window's layer stacks and its key router, wired to the store: the composition root of the layer system (`Sources/Layers/CLAUDE.md`).
///
/// It keeps one stack per tab, for exactly as long as the tab is in the core's state, with the tab's terminal as its bottom layer, under the progress page of a tab creating its workspace.
@MainActor
@Observable
final class Surfaces {
	@ObservationIgnored private(set) var router: LayerRouter!
	private(set) var tabStacks: [UInt64: LayerStack] = [:]
	/// The open Workspace Hub's model; nil while it is closed.
	@ObservationIgnored private(set) var hub: WorkspaceHubModel?
	/// The open cheatsheet's scrolling; nil while it is closed.
	@ObservationIgnored private(set) var cheatsheet: VimScroll?
	/// The open confirmation dialog; nil while none is.
	@ObservationIgnored private(set) var dialog: ConfirmationDialogModel?
	/// The open New Task dialog; nil while none is.
	@ObservationIgnored private(set) var newTask: NewTaskDialogModel?
	/// From the New Task dialog's opening until its repos have loaded, so that a held or repeated `n` opens one dialog.
	@ObservationIgnored private var newTaskOpening = false
	@ObservationIgnored private var hubMenu: LayerHandle?
	@ObservationIgnored private let store: Store

	init(store: Store) {
		self.store = store
		router = LayerRouter { [unowned self] in store.app.window.activeTab.flatMap { tabStacks[$0] } }
		router.shortcuts = AppShortcuts(store: store, windowStack: router.windowStack) { [unowned self] in
			openWorkspaceHub()
		}.handle
	}

	var windowStack: LayerStack { router.windowStack }

	/// Makes a stack for each new tab, with a progress page while the tab creates its workspace, and tears down the stacks of closed tabs.
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
		for tab in tabs where tab.session.kind.isCreation {
			if let stack = tabStacks[tab.id], stack.find(CreationProgressPage.layerName) == nil {
				pushProgressPage(on: stack, tab: tab.id)
			}
		}
	}

	/// Removes the progress page of each tab that has its workspace; its terminal is already beneath it (`pushTerminal`), so the page reveals it.
	func retireProgressPages(of tabs: [AppTab]) {
		for tab in tabs where !tab.session.kind.isCreation {
			if let stack = tabStacks[tab.id], let page = stack.find(CreationProgressPage.layerName) { stack.remove(page) }
		}
	}

	/// A page, so it takes `q` and Escape from the stack's dismissal, and consumes the plain keys it has no use for, which would otherwise reach whatever has the focus, such as another tab's terminal.
	private func pushProgressPage(on stack: LayerStack, tab: UInt64) {
		let store = store
		var spec = pageLayer(LayerSpec(name: CreationProgressPage.layerName) { CreationProgressPage(store: store, tab: tab) })
		spec.onKey = { $0.isUnusedOnAPage }
		spec.onUserDismissRequest = {
			CreationPageState(store: store, tab: tab).closeOrCancel()
			return .handled
		}
		stack.push(spec)
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
			closeHubMenu()
			windowStack.remove(handle)
			store.dispatch(.openWorkspace(workspace))
		}
		model.showCheatsheet = { [unowned self] in openCheatsheet() }
		model.toggleMenu = { [unowned self, unowned model] menu in toggleHubMenu(menu, of: model) }
		model.closeMenu = { [unowned self] in closeHubMenu() }
		model.confirm = { [unowned self] confirmation, action in confirm(confirmation, then: action) }
		model.newTask = { [unowned self] source in openNewTaskDialog(duplicating: source) }
		hub = model
		model.load()
	}

	/// A header dropdown is a popup layer: it routes `q` and Escape to closing it and passes every other key on to the hub, while the hub draws it under its trigger.
	private func toggleHubMenu(_ menu: HubMenu, of model: WorkspaceHubModel) {
		let wasOpen = model.openMenu
		closeHubMenu()
		guard wasOpen != menu else { return }
		var spec = popupLayer(LayerSpec(name: "dropdown-menu") { EmptyView() })
		spec.onCleanup = { [unowned self, unowned model] in
			model.openMenu = nil
			hubMenu = nil
		}
		hubMenu = windowStack.push(spec)
		model.openMenu = menu
	}

	private func closeHubMenu() {
		if let hubMenu { windowStack.remove(hubMenu) }
	}

	/// Shows a confirmation dialog as a modal layer over the window, and runs `action` once the user confirms; cancelling it changes nothing.
	func confirm(_ confirmation: Confirmation, then action: @escaping () -> Void) {
		var handle: LayerHandle?
		let close = { [unowned self] in
			if let handle { windowStack.remove(handle) }
		}
		let model = ConfirmationDialogModel(confirmation, onConfirm: { close(); action() }, onCancel: close)
		var spec = modalLayer(LayerSpec(name: ConfirmationDialogModel.layerName) { ConfirmationDialogView(model: model) })
		spec.onKey = model.handleKey
		spec.cycleFocus = { model.cycleFocus(backward: $0) }
		spec.onCleanup = { [unowned self] in
			if dialog === model { dialog = nil }
		}
		handle = windowStack.push(spec)
		dialog = model
	}

	/// Opens the New Task dialog over the hub, once the selected profile's repos have loaded; with a `source`, it duplicates that workspace's task, with its title and its repos selected.
	/// Creating the task closes the dialog and the hub, then hands the creation to its own tab (`Command.createTask`), as Ember's hand-off does.
	private func openNewTaskDialog(duplicating source: Workspace?) {
		guard store.selectedProfilePath != nil, !newTaskOpening, windowStack.find(NewTaskDialogModel.layerName) == nil else { return }
		newTaskOpening = true
		Task { [unowned self, store] in
			defer { newTaskOpening = false }
			// A failed call has its banner; the dialog still opens, as Ember's does.
			let repos = await store.registeredRepos() ?? []
			// The hub may have closed meanwhile, and the dialog would be stranded over a terminal.
			guard let hubHandle = windowStack.find(WorkspaceHubModel.layerName) else { return }
			var handle: LayerHandle?
			let close = { [unowned self] in
				if let handle { windowStack.remove(handle) }
			}
			let selected = Set(repos.filter { source?.repos.contains($0.name) == true }.map(\.path))
			let router = router!
			let model = NewTaskDialogModel(
				repos: repos, title: source?.title ?? "", selected: selected, isDuplicating: source != nil,
				clearFocus: { router.clearFocus() },
				onCreate: { [unowned self] title, repoPaths in
					close()
					closeHubMenu()
					windowStack.remove(hubHandle)
					store.dispatch(.createTask(title: title, repoPaths: repoPaths, sourceWorkspacePath: source?.path))
				},
				onCancel: close)
			var spec = modalLayer(LayerSpec(name: NewTaskDialogModel.layerName) { NewTaskDialogView(model: model) })
			spec.onKey = model.handleKey
			spec.cycleFocus = { model.cycleFocus(backward: $0) }
			spec.focus = { model.focus = .title }
			spec.onCleanup = { [unowned self] in
				if newTask === model { newTask = nil }
			}
			closeHubMenu()
			handle = windowStack.push(spec)
			newTask = model
		}
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

	static let terminalLayerName = "terminal"

	/// Puts a tab's terminal at the bottom of its stack, as soon as the tab has one, so that it takes the keys before it runs; a tab that created its workspace gets it under its progress page, which `retireProgressPages` then removes.
	func pushTerminal(_ session: TerminalSession, tab: UInt64, onExit: @escaping () -> Void) {
		guard let stack = tabStacks[tab], stack.find(Self.terminalLayerName) == nil else { return }
		var spec = terminalLayer(LayerSpec(name: Self.terminalLayerName) { TerminalTab(session: session, onExit: onExit) })
		spec.focus = { session.focus() }
		stack.insertAtBottom(spec)
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
