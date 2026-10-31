import CoreBindings
import Foundation
import Observation
import Synchronization

public enum Command: Sendable {
	/// Loads the version and the profiles, then the workspaces of the selected (or first) profile.
	case load
	case selectProfile(path: String)
	/// Loads the status of each workspace that has none yet, a few workspaces at a time, each kept as it arrives; `fresh` drops the statuses already loaded first.
	case loadWorkspaceStatuses(fresh: Bool)
	/// Opens a tab with a login shell, outside any profile.
	case openShell
	/// Opens a tab on a workspace, with the workspace's own profile: the selection may have moved on from the profile it was listed for.
	case openWorkspace(Workspace)
	case activateTab(id: UInt64)
	/// Activates the tab after the active one, wrapping around to the first; `activatePreviousTab` goes the other way.
	case activateNextTab
	case activatePreviousTab
	/// Moves the active tab one place towards the end of the tab bar, without wrapping; `moveActiveTabBackward` goes the other way.
	case moveActiveTabForward
	case moveActiveTabBackward
	case closeTab(id: UInt64)
	/// Opens a tab named `title` and creates a workspace for it in the selected profile, with these repos; `sourceWorkspacePath` duplicates that workspace's task.
	case createTask(title: String, repoPaths: [String], sourceWorkspacePath: String?)
	/// Cancels the workspace creation running in a tab; a cancel asked for before the creation has started is held until it has.
	case cancelCreation(tab: UInt64)
	/// Answers `StartupProblem.stateUnreadable`: moves the unreadable app state aside, keeping it, and opens a fresh one in its place.
	case startFresh
	/// Removes an error banner.
	case dismissError(id: UInt64)
}

/// Where the app state (the tabs) is kept.
public enum StateStorage: Sendable {
	/// Forgotten when the store goes.
	case inMemory
	case file(path: String)
	/// The app's own, from `sapphireStatePath`: per install suffix, or in `DEVORA_STATE_DIR`.
	case app
}

/// Why the store couldn't open the app state; it has no core until the problem is resolved.
public enum StartupProblem: Equatable, Sendable {
	/// Another process (another Devora Sapphire) holds the state; this one can only quit.
	case stateInUse(path: String)
	/// The state couldn't be opened; the user either starts fresh (`Command.startFresh`) or quits.
	case stateUnreadable(path: String, reason: String)
}

/// The single app store: views read its state and dispatch `Command`s; only the store calls the core.
@MainActor
@Observable
public final class Store {
	public private(set) var version = ""
	public private(set) var profiles: [Profile] = []
	/// Whether the profiles have been listed once: with none, Ember's first-run welcome shows only after a listing that succeeded.
	public private(set) var profilesListed = false
	public private(set) var selectedProfilePath: String?
	public private(set) var workspaces: [Workspace] = []
	/// The statuses loaded so far, by workspace path; a workspace without one is still loading.
	public private(set) var workspaceStatuses: [String: WorkspaceStatus] = [:]
	/// The core-owned app state (tabs and their sessions), folded from the core's events.
	public private(set) var app = AppState.empty
	public private(set) var startupProblem: StartupProblem?
	public private(set) var errorBanners = ErrorBanners()
	/// The latest listing's latencies, with each status's as it arrives; nil without a selected profile.
	public private(set) var loadingLatencies: LoadingLatencies?
	/// The progress of the workspace creations started in this run, by the tab each runs in, until the tab closes or has its workspace.
	public private(set) var creations: [UInt64: CreationProgress] = [:]

	/// Asks the window's display link to call `foldPending()` on its next frame; without one, events fold on the main actor's next turn.
	@ObservationIgnored public var requestFrame: (@MainActor () -> Void)?

	@ObservationIgnored public let log: AppLog
	@ObservationIgnored private let configPath: String?
	@ObservationIgnored private var core: DevoraCore?
	@ObservationIgnored private let inbox = Inbox()
	@ObservationIgnored private var stateEvents: Task<Void, Never>?
	@ObservationIgnored private var reportedErrors: Task<Void, Never>?
	/// Bumped whenever the statuses are dropped, so that the loads started before can't store theirs.
	@ObservationIgnored private var statusGeneration = 0
	@ObservationIgnored private var statusesLoading: Set<String> = []
	/// When the listing `loadingLatencies` describes started; its times count from here.
	@ObservationIgnored private var latencyOrigin = ContinuousClock.now
	/// Every error shown since the last `scrapeErrors()`, for the acceptance tests, as Ember records them.
	@ObservationIgnored private var recordedErrors: [String] = []
	/// The ids of the creations still running, by tab, for cancelling them.
	@ObservationIgnored private var runningCreations: [UInt64: UInt32] = [:]
	/// The tabs whose creation was cancelled before `create_workspace` returned its id.
	@ObservationIgnored private var cancelsBeforeStart: Set<UInt64> = []

	/// `configPath` overrides the global config file (`nil`: `DEVORA_CONFIG_PATH`, then `~/.config/devora/config.json`).
	/// `log` is where the store and the core write, and where the core reports its errors from.
	public init(configPath: String?, state: StateStorage, log: AppLog) {
		self.configPath = configPath
		self.log = log
		reportedErrors = pullReportedErrors()
		open(state)
	}

	private func open(_ state: StateStorage) {
		do {
			let statePath: String? = switch state {
			case .inMemory: nil
			case .file(let path): path
			case .app: try sapphireStatePath(resourcesDir: Bundle.main.resourcePath)
			}
			let core = try DevoraCore(configPath: configPath, statePath: statePath, log: log)
			self.core = core
			stateEvents = pullStateEvents(from: try core.subscribe())
		} catch CoreError.StateInUse(let path, _) {
			startupProblem = .stateInUse(path: path)
		} catch CoreError.StateUnreadable(let path, let message) {
			startupProblem = .stateUnreadable(path: path, reason: message)
		} catch {
			showError("the core failed to start: \(Self.describe(error))")
		}
	}

	isolated deinit {
		stateEvents?.cancel()
		reportedErrors?.cancel()
	}

	@discardableResult
	public func dispatch(_ command: Command) -> Task<Void, Never> {
		switch command {
		case .load:
			return Task { await load() }
		case .selectProfile(let path):
			return Task {
				guard let core else { return }
				await select(path, core: core)
			}
		case .loadWorkspaceStatuses(let fresh):
			loadWorkspaceStatuses(fresh: fresh)
		case .openShell:
			apply(Self.openShell)
		case .startFresh:
			startFresh()
		case .openWorkspace(let workspace):
			apply(.openSession(
				title: workspace.title.isEmpty ? workspace.id : workspace.title,
				kind: .workspace(path: workspace.path),
				profilePath: workspace.profilePath))
		case .activateTab(let id):
			apply(.activateTab(tab: id))
		case .activateNextTab:
			apply(.activateNext)
		case .activatePreviousTab:
			apply(.activatePrevious)
		case .moveActiveTabForward:
			apply(.moveActiveTabForward)
		case .moveActiveTabBackward:
			apply(.moveActiveTabBackward)
		case .closeTab(let id):
			apply(.closeSession(tab: id))
		case .createTask(let title, let repoPaths, let source):
			return Task { await createTask(title: title, repoPaths: repoPaths, sourceWorkspacePath: source) }
		case .cancelCreation(let tab):
			cancelCreation(tab: tab)
		case .dismissError(let id):
			errorBanners.dismiss(id: id)
		}
		return Task {}
	}

	/// Folds the events that arrived since the last fold into `app`, as one change.
	public func foldPending() {
		let events = inbox.drain()
		guard !events.isEmpty else { return }
		var app = self.app
		for event in events {
			followCreation(event, in: app)
			app.apply(event)
		}
		self.app = app
	}

	/// The launch spec of the terminal with this id, from its tab's session.
	public func terminalSpec(id: UInt64) async throws -> TerminalSpec {
		guard let core else { throw CoreError.Failed(message: "the core isn't running") }
		let resources = Bundle.main.resourcePath
		return try await call("terminal_spec") { try core.terminalSpec(pane: id, resourcesDir: resources) }
	}

	/// Whether the core's app state has no tabs, as after a launch that restored none; read from the core, since its events fold on a later frame.
	public func hasNoTabs() -> Bool {
		guard let core, let snapshot = try? call("snapshot", { try core.snapshot() }) else { return false }
		return snapshot.window.tabs.isEmpty
	}

	/// The repos a task in the selected profile can use; nil when the call failed.
	public func registeredRepos() async -> [RepoInfo]? {
		guard let core, let profile = selectedProfilePath else { return nil }
		return try? await call("get_registered_repos") { try core.getRegisteredRepos(profilePath: profile) }
	}

	/// Ends a workspace's task (`remove_task`: its repos' changes are discarded and their HEADs detached; the worktrees stay), then lists the workspaces again; returns whether the task was removed.
	@discardableResult
	public func removeTask(_ workspace: Workspace) async -> Bool {
		guard let core else { return false }
		let path = workspace.path
		guard (try? await call("remove_task", { try core.removeTask(workspacePath: path) })) != nil else { return false }
		await workspaceChanged(path, core: core)
		return true
	}

	/// Deletes a workspace (`delete_workspace`: its worktrees, then its directory), then lists the workspaces again; returns whether it was deleted.
	@discardableResult
	public func deleteWorkspace(_ workspace: Workspace) async -> Bool {
		guard let core else { return false }
		let path = workspace.path
		guard (try? await call("delete_workspace", { try core.deleteWorkspace(workspacePath: path) })) != nil else { return false }
		await workspaceChanged(path, core: core)
		return true
	}

	/// What the profile form's root path would be (`validate_profile_path`); nil when the call failed.
	public func validateProfilePath(_ path: String) async -> ProfilePathValidation? {
		guard let core else { return nil }
		return try? await call("validate_profile_path") { try core.validateProfilePath(path: path) }
	}

	/// Registers the profile at `path` (`register_profile`): an existing profile as it is, or a new one, created there with `name`; nil when the call failed.
	public func registerProfile(path: String, name: String) async -> RegisteredProfile? {
		guard let core else { return nil }
		return try? await call("register_profile") { try core.registerProfile(path: path, name: name) }
	}

	/// Removes a profile from the registry (`unregister_profile`), leaving its directory on disk, then loads again, which selects the first profile if it was the selected one; returns whether it was removed.
	/// Ember refuses while tabs use the profile (`tabs(usingProfile:)`), which is the caller's to check.
	@discardableResult
	public func unregisterProfile(path: String) async -> Bool {
		guard let core, (try? await call("unregister_profile", { try core.unregisterProfile(path: path) })) != nil else { return false }
		await load()
		return true
	}

	/// The open tabs whose session belongs to the profile.
	public func tabs(usingProfile path: String) -> [AppTab] {
		app.window.tabs.filter { $0.session.profilePath == path }
	}

	/// Writes the latest load's latencies to `<profile>/diagnostics/hub-profile-<timestamp>.json` and returns the file's path; nil when there is nothing to write or the write failed.
	public func saveLoadingLatencies() async -> String? {
		guard let core, let latencies = loadingLatencies else { return nil }
		let encoder = JSONEncoder()
		encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
		guard let data = try? encoder.encode(latencies) else { return nil }
		let json = String(decoding: data, as: UTF8.self)
		let profile = latencies.profilePath
		return try? await call("save_profiling_report") { try core.saveProfilingReport(profilePath: profile, reportJson: json) }
	}

	/// THE way to surface an error, as Ember's `showError`: recorded for the tests, written to the log, and shown on a banner.
	public func showError(_ message: String) {
		try? log.write(level: .error, message: message)
		banner(message)
	}

	/// Writes a problem that degrades rather than fails to the log, with no banner.
	public func logWarning(_ message: String) {
		try? log.write(level: .warn, message: message)
	}

	/// The errors shown since the last call, which it clears.
	public func scrapeErrors() -> [String] {
		defer { recordedErrors.removeAll() }
		return recordedErrors
	}

	private func banner(_ message: String) {
		recordedErrors.append(message)
		errorBanners.show(message)
	}

	private static let openShell = StateCommand.openSession(title: "Shell", kind: .shell, profilePath: nil)

	/// The core opens the creation's tab and records its outcome in the app state; the store follows its progress for the progress page.
	private func createTask(title: String, repoPaths: [String], sourceWorkspacePath: String?) async {
		guard let core, let profile = selectedProfilePath,
		      let creation = try? await call("create_workspace", {
		      	try core.createWorkspace(profilePath: profile, repoPaths: repoPaths, taskName: title, sourceWorkspacePath: sourceWorkspacePath)
		      })
		else { return }
		let tab = creation.tab
		creations[tab] = CreationProgress()
		runningCreations[tab] = creation.id
		if cancelsBeforeStart.remove(tab) != nil { cancelCreation(tab: tab) }
		let events = creation.events
		Task.detached { [weak self] in
			do {
				for try await batch in Batches(events) {
					await self?.creationProgressed(tab: tab, batch)
					if batch.contains(where: \.isLast) { break }
				}
			} catch {
				await self?.showError("the progress of task creation stopped: \(Self.describe(error))")
			}
			try? events.close()
		}
	}

	/// Creation events are a handful per second at most (the prepare-command's output aside, which comes in batches), so each batch hops to the main actor as it comes.
	private func creationProgressed(tab: UInt64, _ events: [CreationEvent]) {
		if events.contains(where: \.isLast) { runningCreations[tab] = nil }
		guard var progress = creations[tab] else { return }
		for event in events { progress.apply(event) }
		creations[tab] = progress
	}

	private func cancelCreation(tab: UInt64) {
		guard let core else { return }
		guard let id = runningCreations[tab] else {
			// A creation the store has no progress for hasn't returned its id yet; one it has has ended.
			if creations[tab] == nil { cancelsBeforeStart.insert(tab) }
			return
		}
		try? call("cancel_workspace_creation") { try core.cancelWorkspaceCreation(id: id) }
	}

	/// A creation's progress goes once its tab has its workspace or is closed; closing the tab of a creation that is running, or has yet to return its id, cancels it.
	/// `app` is the state before the event, where a closed tab still is.
	private func followCreation(_ event: StateEvent, in app: AppState) {
		switch event {
		case .sessionChanged(let tab, let session):
			if case .workspace = session.kind { creations[tab] = nil }
		case .sessionClosed(let tab):
			if app.window.tabs.first(where: { $0.id == tab })?.session.kind == .creatingWorkspace { cancelCreation(tab: tab) }
			creations[tab] = nil
		default:
			break
		}
	}

	private func startFresh() {
		guard case .stateUnreadable(let path, _) = startupProblem,
		      let aside = try? call("set_aside_state", { try setAsideState(path: path) })
		else { return }
		logWarning("Moved the unreadable app state aside to \(aside)")
		startupProblem = nil
		open(.file(path: path))
	}

	/// State commands apply in order and quickly (a reducer and one small SQLite transaction), so they run synchronously; their changes arrive through the subscription.
	private func apply(_ command: StateCommand) {
		guard let core else { return }
		try? call("dispatch") { try core.dispatch(command: command) }
	}

	private func pullStateEvents(from subscription: StateSubscription) -> Task<Void, Never> {
		let inbox = inbox
		return Task.detached { [weak self] in
			do {
				for try await batch in Batches(subscription) {
					if inbox.append(batch) { await self?.eventsArrived() }
				}
			} catch {
				await self?.showError("the app state's events stopped: \(Self.describe(error))")
			}
		}
	}

	private func eventsArrived() {
		if let requestFrame { requestFrame() } else { foldPending() }
	}

	/// The core already wrote the errors it reports to the log, so they only need their banners; they are rare, so each batch hops to the main actor as it comes.
	private func pullReportedErrors() -> Task<Void, Never>? {
		guard let subscription = try? call("errors", { try log.errors() }) else { return nil }
		return Task.detached { [weak self] in
			do {
				for try await batch in Batches(subscription) {
					await self?.bannerAll(batch)
				}
			} catch {
				await self?.showError("the core's error reports stopped: \(Self.describe(error))")
			}
		}
	}

	private func bannerAll(_ messages: [String]) {
		for message in messages { banner(message) }
	}

	/// Loads the version and the profiles, then the workspaces of the selected (or first) profile; returns whether every call succeeded.
	/// `selecting` selects that profile instead, when it is listed.
	@discardableResult
	public func load(selecting preferred: String? = nil) async -> Bool {
		guard let core, let version = try? await call("version", { try core.version() }) else { return false }
		let started = ContinuousClock.now
		guard let profiles = try? await call("list_profiles", { try core.listProfiles() }) else { return false }
		let listProfiles = LoadingLatencies.Phase(from: started, to: .now, origin: started)
		self.version = version
		self.profiles = profiles
		profilesListed = true
		let selected = profiles.first { $0.path == preferred } ?? profiles.first { $0.path == selectedProfilePath } ?? profiles.first
		return await select(selected?.path, core: core, started: started, listProfiles: listProfiles)
	}

	@discardableResult
	private func select(
		_ path: String?, core: DevoraCore, started: ContinuousClock.Instant = .now,
		listProfiles: LoadingLatencies.Phase = .zero
	) async -> Bool {
		if path != selectedProfilePath { dropWorkspaceStatuses() }
		selectedProfilePath = path
		guard let path else {
			workspaces = []
			loadingLatencies = nil
			return true
		}
		let listStarted = ContinuousClock.now
		let list = try? await call("list_workspaces") { try core.listWorkspaces(profilePath: path) }
		// A later selection wins over a slower, earlier one.
		guard selectedProfilePath == path else { return false }
		workspaces = list ?? []
		let listed = ContinuousClock.now
		latencyOrigin = started
		loadingLatencies = LoadingLatencies(
			timestamp: Date.now.ISO8601Format(),
			profileName: profiles.first { $0.path == path }?.name ?? "",
			profilePath: path,
			workspaceCount: workspaces.count,
			phases: .init(
				listProfiles: listProfiles,
				listWorkspaces: .init(from: listStarted, to: listed, origin: started),
				totalLoad: .init(from: started, to: listed, origin: started)),
			workspaceStatuses: [])
		return list != nil
	}

	/// After a workspace's task was removed or the workspace deleted: its status is stale and the list may have changed.
	private func workspaceChanged(_ path: String, core: DevoraCore) async {
		workspaceStatuses[path] = nil
		guard let profile = selectedProfilePath,
		      let list = try? await call("list_workspaces", { try core.listWorkspaces(profilePath: profile) }),
		      selectedProfilePath == profile
		else { return }
		workspaces = list
	}

	private func dropWorkspaceStatuses() {
		workspaceStatuses = [:]
		statusesLoading = []
		statusGeneration += 1
	}

	private nonisolated static let concurrentStatusLoads = 4

	/// Each workspace's repos are read in parallel, so taking a few workspaces at a time bounds the processes, as Ember's single batch does.
	private func loadWorkspaceStatuses(fresh: Bool) {
		guard let core else { return }
		if fresh { dropWorkspaceStatuses() }
		// The active ones first, since the hub opens on them.
		let pending = (workspaces.filter(\.active) + workspaces.filter { !$0.active })
			.filter { workspaceStatuses[$0.path] == nil && !statusesLoading.contains($0.path) }
		guard !pending.isEmpty else { return }
		statusesLoading.formUnion(pending.map(\.path))
		let generation = statusGeneration
		let requests = pending.map { StatusRequest(path: $0.path, repos: $0.repos) }
		Task.detached { [weak self] in
			await withTaskGroup(of: StatusLoad.self) { group in
				var next = requests.makeIterator()
				func start(_ request: StatusRequest) {
					group.addTask {
						let started = ContinuousClock.now
						let result = Result { try core.workspaceStatus(workspacePath: request.path, repoNames: request.repos) }
						return StatusLoad(request: request, result: result, started: started, finished: .now)
					}
				}
				for _ in 0..<Self.concurrentStatusLoads {
					guard let request = next.next() else { break }
					start(request)
				}
				for await load in group {
					guard let self, await self.statusArrived(load, generation: generation) else { return }
					if let request = next.next() { start(request) }
				}
			}
		}
	}

	/// Keeps a workspace's status, and its latency; returns false once its load was superseded, so that it starts no more.
	private func statusArrived(_ load: StatusLoad, generation: Int) -> Bool {
		guard generation == statusGeneration else { return false }
		let path = load.request.path
		statusesLoading.remove(path)
		let status: WorkspaceStatus
		switch load.result {
		case .success(let loaded):
			status = loaded
		case .failure(let error):
			showError("workspace_status failed: \(Self.describe(error))")
			status = WorkspaceStatus(repos: [], error: Self.describe(error))
		}
		workspaceStatuses[path] = status
		if let id = workspaces.first(where: { $0.path == path })?.id {
			let phase = LoadingLatencies.Phase(from: load.started, to: load.finished, origin: latencyOrigin)
			loadingLatencies?.workspaceStatuses.append(.init(
				wsId: id, repoCount: load.request.repos.count, startMs: phase.startMs, durationMs: phase.durationMs, error: status.error))
		}
		return true
	}

	/// A core call, whose failure is shown as `<name> failed: <error>`, then rethrown for the caller's flow.
	private func call<T>(_ name: String, _ body: () throws -> T) throws -> T {
		do {
			return try body()
		} catch {
			throw shown(error, of: name)
		}
	}

	/// Like the synchronous `call`, but off the main actor, since most core calls do file I/O.
	private func call<T: Sendable>(_ name: String, _ body: @escaping @Sendable () throws -> T) async throws -> T {
		do {
			return try await Task.detached(operation: body).value
		} catch {
			throw shown(error, of: name)
		}
	}

	private func shown(_ error: any Error, of call: String) -> any Error {
		showError("\(call) failed: \(Self.describe(error))")
		return error
	}

	public nonisolated static func describe(_ error: any Error) -> String {
		switch error as? CoreError {
		case .Failed(let message), .StateInUse(_, let message), .StateUnreadable(_, let message): message
		case nil: String(describing: error)
		}
	}
}

private struct StatusRequest: Sendable {
	let path: String
	let repos: [String]
}

private struct StatusLoad: Sendable {
	let request: StatusRequest
	let result: Result<WorkspaceStatus, any Error>
	let started: ContinuousClock.Instant
	let finished: ContinuousClock.Instant
}

/// Events buffered off the main actor until the next fold.
private final class Inbox: Sendable {
	private let events = Mutex<[StateEvent]>([])

	/// Returns whether the inbox was empty, that is, whether a fold needs to be requested.
	func append(_ batch: [StateEvent]) -> Bool {
		events.withLock { events in
			defer { events.append(contentsOf: batch) }
			return events.isEmpty
		}
	}

	func drain() -> [StateEvent] {
		events.withLock { events in
			defer { events.removeAll(keepingCapacity: true) }
			return events
		}
	}
}
