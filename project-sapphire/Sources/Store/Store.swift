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
	/// Opens a tab on a workspace of the selected profile.
	case openWorkspace(Workspace)
	/// Opens a shell tab when there are no tabs, as at a launch that restored none.
	case openShellIfNoTabs
	case activateTab(id: UInt64)
	/// Activates the tab after the active one, wrapping around to the first; `activatePreviousTab` goes the other way.
	case activateNextTab
	case activatePreviousTab
	/// Moves the active tab one place towards the end of the tab bar, without wrapping; `moveActiveTabBackward` goes the other way.
	case moveActiveTabForward
	case moveActiveTabBackward
	case closeTab(id: UInt64)
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
	public private(set) var selectedProfilePath: String?
	public private(set) var workspaces: [Workspace] = []
	/// The statuses loaded so far, by workspace path; a workspace without one is still loading.
	public private(set) var workspaceStatuses: [String: WorkspaceStatus] = [:]
	/// The core-owned app state (tabs and their sessions), folded from the core's events.
	public private(set) var app = AppState.empty
	public private(set) var startupProblem: StartupProblem?
	public private(set) var errorBanners = ErrorBanners()

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
	/// Every error shown since the last `scrapeErrors()`, for the acceptance tests, as Ember records them.
	@ObservationIgnored private var recordedErrors: [String] = []

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
		case .openShellIfNoTabs:
			guard let core, let snapshot = try? call("snapshot", { try core.snapshot() }) else { break }
			if snapshot.window.tabs.isEmpty { apply(Self.openShell) }
		case .startFresh:
			startFresh()
		case .openWorkspace(let workspace):
			apply(.openSession(
				title: workspace.title.isEmpty ? workspace.id : workspace.title,
				kind: .workspace(path: workspace.path),
				profilePath: selectedProfilePath))
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
		for event in events { app.apply(event) }
		self.app = app
	}

	/// The launch spec of the terminal with this id, from its tab's session.
	public func terminalSpec(id: UInt64) async throws -> TerminalSpec {
		guard let core else { throw CoreError.Failed(message: "the core isn't running") }
		let resources = Bundle.main.resourcePath
		return try await call("terminal_spec") { try core.terminalSpec(pane: id, resourcesDir: resources) }
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
	@discardableResult
	public func load() async -> Bool {
		guard let core,
		      let version = try? await call("version", { try core.version() }),
		      let profiles = try? await call("list_profiles", { try core.listProfiles() })
		else { return false }
		self.version = version
		self.profiles = profiles
		let selected = profiles.first { $0.path == selectedProfilePath } ?? profiles.first
		return await select(selected?.path, core: core)
	}

	@discardableResult
	private func select(_ path: String?, core: DevoraCore) async -> Bool {
		if path != selectedProfilePath { dropWorkspaceStatuses() }
		selectedProfilePath = path
		guard let path else {
			workspaces = []
			return true
		}
		let list = try? await call("list_workspaces") { try core.listWorkspaces(profilePath: path) }
		// A later selection wins over a slower, earlier one.
		guard selectedProfilePath == path else { return false }
		workspaces = list ?? []
		return list != nil
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
			await withTaskGroup(of: (String, Result<WorkspaceStatus, any Error>).self) { group in
				var next = requests.makeIterator()
				func start(_ request: StatusRequest) {
					group.addTask {
						(request.path, Result { try core.workspaceStatus(workspacePath: request.path, repoNames: request.repos) })
					}
				}
				for _ in 0..<Self.concurrentStatusLoads {
					guard let request = next.next() else { break }
					start(request)
				}
				for await (path, result) in group {
					guard let self, await self.statusArrived(path: path, result: result, generation: generation) else { return }
					if let request = next.next() { start(request) }
				}
			}
		}
	}

	/// Keeps a workspace's status; returns false once its load was superseded, so that it starts no more.
	private func statusArrived(path: String, result: Result<WorkspaceStatus, any Error>, generation: Int) -> Bool {
		guard generation == statusGeneration else { return false }
		statusesLoading.remove(path)
		switch result {
		case .success(let status):
			workspaceStatuses[path] = status
		case .failure(let error):
			showError("workspace_status failed: \(Self.describe(error))")
			workspaceStatuses[path] = WorkspaceStatus(repos: [], error: Self.describe(error))
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
