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
	@ObservationIgnored private(set) var shortcuts: AppShortcuts!
	private(set) var agentPanels: [UInt64: AgentPanelModel] = [:]
	private(set) var tabStacks: [UInt64: LayerStack] = [:]
	/// Transient preview rendering models, keyed by the core-owned pane ID.
	private(set) var previewModels: [UInt64: PreviewPaneModel] = [:]
	/// The open Workspace Hub's model; nil while it is closed.
	@ObservationIgnored private(set) var hub: WorkspaceHubModel?
	/// The open Agent Hub's model; nil while it is closed.
	@ObservationIgnored private(set) var agentHub: AgentHubModel?
	/// The open New Agent dialog; nil while none is.
	@ObservationIgnored private(set) var newAgent: NewAgentDialogModel?
	/// The open cheatsheet's scrolling; nil while it is closed.
	@ObservationIgnored private(set) var cheatsheet: VimScroll?
	/// The open confirmation dialog; nil while none is.
	@ObservationIgnored private(set) var dialog: ConfirmationDialogModel?
	/// The open New Task dialog; nil while none is.
	@ObservationIgnored private(set) var newTask: NewTaskDialogModel?
	/// The open Settings Hub; nil while it is closed.
	@ObservationIgnored private(set) var settings: SettingsHubModel?
	/// The open Health Hub; nil while it is closed.
	@ObservationIgnored private(set) var health: HealthHubModel?
	/// The open guide's rendering model; observed by the test bridge.
	@ObservationIgnored private(set) var userGuide: UserGuideModel?
	/// The open Command Palette; nil while it is closed.
	@ObservationIgnored private(set) var palette: CommandPaletteModel?
	/// The open Repurpose dialog; nil while none is.
	@ObservationIgnored private(set) var repurposeDialog: RepurposeDialogModel?
	/// The open Add Repo dialog; nil while none is.
	@ObservationIgnored private(set) var addRepo: AddRepoDialogModel?
	/// The open Clone Repo dialog; nil while none is.
	@ObservationIgnored private(set) var cloneRepo: CloneRepoDialogModel?
	/// From the New Task dialog's opening until its repos have loaded, so that a held or repeated `n` opens one dialog.
	@ObservationIgnored private var newTaskOpening = false
	/// The same for the Add Repo and Clone Repo dialogs.
	@ObservationIgnored private var addRepoOpening = false
	@ObservationIgnored private var cloneRepoOpening = false
	@ObservationIgnored private var newAgentOpening = false
	@ObservationIgnored private var hubMenu: LayerHandle?
	@ObservationIgnored private let store: Store

	init(store: Store) {
		self.store = store
		router = LayerRouter { [unowned self] in store.app.window.activeTab.flatMap { tabStacks[$0] } }
		shortcuts = AppShortcuts(store: store, windowStack: router.windowStack,
			hubs: { [unowned self] in hubShortcut() },
			openUserGuide: { [unowned self] in openUserGuide() })
		router.shortcuts = shortcuts.handle
		shortcuts.openCommandPalette = { [unowned self] in openCommandPalette() }
	}

	var windowStack: LayerStack { router.windowStack }

	/// Opens immediately, then refreshes only the profile rows without delaying the search field.
	func openCommandPalette() {
		guard windowStack.find(CommandPaletteModel.layerName) == nil else { return }
		let model = CommandPaletteModel(commands: PaletteCommand.commands(profiles: store.profiles, active: store.selectedProfilePath, globalPaused: store.app.globalPaused))
		model.isEditingText = { [unowned self] in router.isEditingText }
		var spec = pageLayer(LayerSpec(name: CommandPaletteModel.layerName) { CommandPaletteView(model: model) })
		spec.onKey = model.handleKey
		spec.focus = { model.fieldFocused = true }
		spec.onCleanup = { [unowned self] in if palette === model { palette = nil } }
		// SwiftUI mounts the field on the next turn; the previous terminal must not receive keys in between.
		router.clearFocus()
		let handle = windowStack.push(spec)
		model.close = { [unowned self] in windowStack.remove(handle) }
		model.run = { [unowned self] action in runPaletteCommand(action) }
		palette = model
		Task { [weak self, weak model, store] in
			guard await store.listProfiles(), let self, let model, self.palette === model else { return }
			model.updateProfiles(store.profiles, active: store.selectedProfilePath, globalPaused: store.app.globalPaused)
		}
	}

	private func runPaletteCommand(_ action: PaletteCommand.Action) {
		switch action {
		case .runRepoUpdates: Task { await store.runRepoUpdates() }
		case .toggleGlobalPause: store.dispatch(.setGlobalPaused(!store.app.globalPaused))
		case .agentHub: openAgentHub()
		case .workspaceHub: openWorkspaceHub()
		case .newShell: store.dispatch(.openShell)
		case .addRepo:
			if let target = store.addRepoTargetOfActiveTab() { openAddRepoDialog(for: target) {} }
		case .cloneRepo:
			guard let profile = store.selectedProfilePath else {
				store.showError("No active profile — open the Workspace Hub and set one first")
				return
			}
			openCloneRepoDialog(profilePath: profile) { _ in }
		case .closeSession:
			if let tab = store.app.window.activeTab { store.dispatch(.closeTab(id: tab)) }
		case .health: openHealthHub()
		case .newProfile: openSettingsHub(.newProfile)
		case .settings: openSettingsHub(.list)
		case .switchProfile(let path):
			Task { [unowned self, store] in
				await store.dispatch(.selectProfile(path: path)).value
				openWorkspaceHub()
			}
		case .duplicate: duplicateCurrentSession()
		case .repurpose: repurposeCurrentSession()
		}
	}

	private func duplicateCurrentSession() {
		let window = store.app.window
		guard let tab = window.tabs.first(where: { $0.id == window.activeTab }),
		      case .workspace(let path) = tab.session.kind, let profile = tab.session.profilePath else {
			store.showError("Cannot duplicate: the current session has no associated workspace")
			return
		}
		Task { [unowned self, store] in
			await store.dispatch(.selectProfile(path: profile)).value
			guard let workspace = store.workspaces.first(where: { $0.path == path }) else {
				store.showError("Cannot duplicate: the current workspace could not be found")
				return
			}
			openWorkspaceHub()
			openNewTaskDialog(duplicating: workspace)
		}
	}

	private func repurposeCurrentSession() {
		let window = store.app.window
		guard let tab = window.tabs.first(where: { $0.id == window.activeTab }),
		      case .workspace(let path) = tab.session.kind else {
			store.showError("Cannot repurpose: the current session has no associated workspace")
			return
		}
		let id = tab.id
		Task { [unowned self, store] in
			guard let title = await store.prepareRepurpose(workspacePath: path),
			      windowStack.find(RepurposeDialogModel.layerName) == nil else { return }
			let model = RepurposeDialogModel(title: title)
			model.clearFocus = { [unowned self] in router.clearFocus() }
			var spec = modalLayer(LayerSpec(name: RepurposeDialogModel.layerName) { RepurposeDialogView(model: model) })
			spec.onKey = model.handleKey
			spec.cycleFocus = { model.cycleFocus(backward: $0) }
			spec.focus = { model.focus = .title }
			spec.onCleanup = { [unowned self] in if repurposeDialog === model { repurposeDialog = nil } }
			let handle = windowStack.push(spec)
			model.close = { [unowned self] in windowStack.remove(handle) }
			model.submit = { [store] title in Task { await store.repurpose(workspacePath: path, title: title, tab: id) } }
			repurposeDialog = model
		}
	}

	/// Makes a stack for each new tab, with a progress page while the tab creates its workspace, and tears down the stacks of closed tabs.
	func sync(with tabs: [AppTab]) {
		let previewIDs = Set(tabs.flatMap { $0.previews.map(\.id) })
		previewModels = previewModels.filter { previewIDs.contains($0.key) }
		for id in previewIDs where previewModels[id] == nil { previewModels[id] = PreviewPaneModel() }
		let live = Set(tabs.map(\.id))
		for (id, stack) in tabStacks where !live.contains(id) {
			stack.clear()
			tabStacks[id] = nil
			agentPanels[id] = nil
		}
		for tab in tabs where tabStacks[tab.id] == nil {
			let id = tab.id
			tabStacks[id] = router.makeTabStack { [store] in store.app.window.activeTab == id }
		}
		for tab in tabs {
			guard case .agentSpace = tab.session.kind, let stack = tabStacks[tab.id],
			      stack.find(AgentChatPanel.layerName) == nil else { continue }
			guard let chat = store.agentChat(tab: tab.id) else { continue }
			let model = AgentPanelModel(chat: chat, store: store)
			agentPanels[tab.id] = model
			model.onEditorReady = { [weak self] in self?.router.focusActiveTab() }
			var spec = pageLayer(LayerSpec(name: AgentChatPanel.layerName) { AgentChatPanel(model: model) })
			spec.onKey = model.handle
			spec.focus = model.focus
			spec.onCleanup = { [store] in chat.flushDraft(); store.forgetAgentChat(tab: chat.tab) }
			// Closing the agent is closing its tab, which pauses it (ADR-012).
			spec.onUserDismissRequest = { .veto }
			stack.push(spec)
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
		spec.onUserDismissRequest = model.dismissDecision
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
		model.openSettings = { [unowned self] opening in
			closeHubMenu()
			openSettingsHub(opening)
		}
		model.openHealth = { [unowned self] in
			closeHubMenu()
			openHealthHub()
		}
		model.addRepo = { [unowned self, unowned model] workspace in
			closeHubMenu()
			openAddRepoDialog(for: AddRepoTarget(workspace)) { [weak model] in model?.repoAdded() }
		}
		model.switchHub = { [unowned self] in switchHubs() }
		hub = model
		model.load()
	}

	/// `Ctrl+S` (ADR-012): inside a hub it switches to the other one; otherwise it opens the Agent Hub, unless another window surface is open.
	func hubShortcut() {
		switch windowStack.top?.spec.name {
		case AgentHubModel.layerName, WorkspaceHubModel.layerName:
			switchHubs()
		default:
			if !windowStack.hasOpaqueLayer { openAgentHub() }
		}
	}

	/// Replaces the open hub with the other one; the first-run welcome, which can't be closed, stays.
	private func switchHubs() {
		if let handle = windowStack.find(AgentHubModel.layerName) {
			windowStack.remove(handle)
			openWorkspaceHub()
		} else if let hub, let handle = windowStack.find(WorkspaceHubModel.layerName), hub.dismissDecision() == .close {
			closeHubMenu()
			windowStack.remove(handle)
			openAgentHub()
		}
	}

	/// Opens the Agent Hub over the window and lists the selected profile's agents; a page closing over it lists them again.
	func openAgentHub() {
		guard windowStack.find(AgentHubModel.layerName) == nil else { return }
		let model = AgentHubModel(store: store)
		var spec = pageLayer(LayerSpec(name: AgentHubModel.layerName) { AgentHubView(model: model) })
		spec.onKey = model.handleKey
		spec.onReveal = model.load
		spec.onCleanup = { [unowned self] in
			model.close()
			if agentHub === model { agentHub = nil }
		}
		let handle = windowStack.push(spec)
		model.open = { [unowned self, store] space in
			windowStack.remove(handle)
			Task { await store.openAgentSpace(space.name) }
		}
		model.newAgent = { [unowned self] in openNewAgentDialog() }
		model.switchHub = { [unowned self] in switchHubs() }
		model.confirm = { [unowned self] confirmation, action in confirm(confirmation, then: action) }
		agentHub = model
		model.load()
	}

	/// Opens the New Agent dialog over the Agent Hub once the profile's repos have loaded; a created agent opens in its tab, closing the hub.
	private func openNewAgentDialog() {
		guard windowStack.find(NewAgentDialogModel.layerName) == nil, !newAgentOpening else { return }
		newAgentOpening = true
		Task { [unowned self, store] in
			defer { newAgentOpening = false }
			guard let repos = await store.registeredRepos() else { return }
			let model = NewAgentDialogModel(store: store, repos: repos, clearFocus: { [unowned self] in router.clearFocus() })
			var spec = modalLayer(LayerSpec(name: NewAgentDialogModel.layerName) { NewAgentDialogView(model: model) })
			spec.onKey = model.handleKey
			spec.cycleFocus = { model.cycleFocus(backward: $0) }
			spec.focus = { model.focus = .name }
			spec.onUserDismissRequest = {
				model.cancel()
				return .handled
			}
			spec.onCleanup = { [unowned self] in if newAgent === model { newAgent = nil } }
			let handle = windowStack.push(spec)
			model.close = { [unowned self] in windowStack.remove(handle) }
			model.opened = { [unowned self, store] name in
				windowStack.remove(handle)
				if let hub = windowStack.find(AgentHubModel.layerName) { windowStack.remove(hub) }
				Task { await store.openAgentSpace(name) }
			}
			newAgent = model
			model.refreshFields()
		}
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
			if let profile = store.selectedProfilePath {
				// After a clone, the list is the profile's repos again, with the clone selected, as Ember's `refreshAfterClone` does.
				model.cloneRepo = { [unowned self, unowned model, store] in
					openCloneRepoDialog(profilePath: profile) { [weak model] cloned in
						Task {
							guard let repos = await store.registeredRepos(inProfile: profile) else { return }
							model?.reposChanged(repos, selecting: cloned.path)
						}
					}
				}
			}
			closeHubMenu()
			handle = windowStack.push(spec)
			newTask = model
		}
	}

	/// Opens the Add Repo dialog over the window for a workspace, once the repos of the workspace's profile have loaded; a profile with none has a banner instead, as in Ember.
	/// `onAdded` runs once a repo was added, after the store has listed the workspaces again.
	func openAddRepoDialog(for target: AddRepoTarget, onAdded: @escaping () -> Void) {
		guard !addRepoOpening, windowStack.find(AddRepoDialogModel.layerName) == nil else { return }
		addRepoOpening = true
		Task { [unowned self, store] in
			defer { addRepoOpening = false }
			guard let repos = await store.registeredRepos(inProfile: target.profilePath) else { return }
			guard !repos.isEmpty else {
				store.showError("No repos are registered in this profile")
				return
			}
			let router = router!
			var handle: LayerHandle?
			let close = { [unowned self] in
				if let handle { windowStack.remove(handle) }
			}
			let model = AddRepoDialogModel(
				target: target, repos: repos, store: store, clearFocus: { router.clearFocus() }, close: close, onAdded: onAdded)
			var spec = modalLayer(LayerSpec(name: AddRepoDialogModel.layerName) { AddRepoDialogView(model: model) })
			spec.onKey = model.handleKey
			spec.onUserDismissRequest = {
				model.dismiss()
				return .handled
			}
			spec.cycleFocus = { model.cycleFocus(backward: $0) }
			spec.focus = { model.focus = .filter }
			spec.onCleanup = { [unowned self] in
				if addRepo === model { addRepo = nil }
			}
			handle = windowStack.push(spec)
			addRepo = model
		}
	}

	/// Opens the Clone Repo dialog over the window, cloning into the profile; `onCloned` runs once the repo was cloned, with its path and name.
	/// It opens on the main actor's next turn: a SwiftUI field made while AppKit dispatches the key that opens its dialog (Space on "+ Clone Repo") never gets the focus it is given.
	func openCloneRepoDialog(profilePath: String, onCloned: @escaping (CreatedWorkspace) -> Void) {
		guard !cloneRepoOpening, windowStack.find(CloneRepoDialogModel.layerName) == nil else { return }
		cloneRepoOpening = true
		Task { [unowned self] in
			cloneRepoOpening = false
			pushCloneRepoDialog(profilePath: profilePath, onCloned: onCloned)
		}
	}

	private func pushCloneRepoDialog(profilePath: String, onCloned: @escaping (CreatedWorkspace) -> Void) {
		let router = router!
		var handle: LayerHandle?
		let close = { [unowned self] in
			if let handle { windowStack.remove(handle) }
		}
		let model = CloneRepoDialogModel(
			profilePath: profilePath, store: store, clearFocus: { router.clearFocus() }, close: close, onCloned: onCloned)
		var spec = modalLayer(LayerSpec(name: CloneRepoDialogModel.layerName) { CloneRepoDialogView(model: model) })
		spec.onKey = model.handleKey
		spec.onUserDismissRequest = {
			model.dismiss()
			return .handled
		}
		spec.cycleFocus = { model.cycleFocus(backward: $0) }
		spec.focus = { model.focus = .url }
		spec.onCleanup = { [unowned self] in
			if cloneRepo === model { cloneRepo = nil }
		}
		handle = windowStack.push(spec)
		cloneRepo = model
	}

	/// Opens the Settings Hub over the Workspace Hub, a page, so that closing it reloads the hub, which lists the active profile's workspaces.
	private func openSettingsHub(_ opening: SettingsHubModel.Opening) {
		guard windowStack.find(SettingsHubModel.layerName) == nil else { return }
		let router = router!
		let model = SettingsHubModel(
			store: store, opening: opening, isEditingText: { router.isEditingText }, clearFocus: { router.clearFocus() })
		var spec = pageLayer(LayerSpec(name: SettingsHubModel.layerName) { SettingsHubView(model: model) })
		spec.onKey = model.handleKey
		spec.onCleanup = { [unowned self] in
			if settings === model { settings = nil }
		}
		let handle = windowStack.push(spec)
		model.close = { [unowned self] in windowStack.remove(handle) }
		model.confirm = { [unowned self] confirmation, action in confirm(confirmation, then: action) }
		model.cloneRepo = { [unowned self] profilePath, onCloned in
			openCloneRepoDialog(profilePath: profilePath) { _ in onCloned() }
		}
		settings = model
		model.load()
	}

	/// Runs Health on a page above the Workspace Hub and cancels it on dismissal.
	private func openHealthHub() {
		guard windowStack.find(HealthHubModel.layerName) == nil else { return }
		let model = HealthHubModel(store: store)
		var spec = pageLayer(LayerSpec(name: HealthHubModel.layerName) { HealthHubView(model: model) })
		spec.onKey = model.handleKey
		spec.onCleanup = { [unowned self] in
			model.close()
			if health === model { health = nil }
		}
		windowStack.push(spec)
		health = model
		model.rerun()
	}

	/// Opens the guide above any window surface, without stacking duplicates.
	private func openUserGuide() {
		guard windowStack.find(UserGuideModel.layerName) == nil else { return }
		let model = UserGuideModel(store: store)
		userGuide = model
		var spec = pageLayer(LayerSpec(name: UserGuideModel.layerName) { UserGuideView(model: model) })
		spec.onKey = model.handleKey
		spec.onCleanup = { [unowned self] in userGuide = nil }
		windowStack.push(spec)
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
	let shortcuts: AppShortcuts

	func makeNSView(context: Context) -> InstallerView {
		InstallerView(router: router, shortcuts: shortcuts)
	}

	func updateNSView(_ view: InstallerView, context: Context) {}

	final class InstallerView: NSView {
		private let router: LayerRouter
		private let shortcuts: AppShortcuts

		init(router: LayerRouter, shortcuts: AppShortcuts) {
			self.router = router
			self.shortcuts = shortcuts
			super.init(frame: .zero)
		}

		@available(*, unavailable)
		required init?(coder: NSCoder) { fatalError() }

		override func viewDidMoveToWindow() {
			super.viewDidMoveToWindow()
			if let window {
				router.install(in: window)
				shortcuts.install(in: window)
			} else {
				router.uninstall()
				shortcuts.uninstall()
			}
		}
	}
}
