import CoreBindings
import Foundation
import Observation
import Synchronization

public enum Command: Sendable {
	/// Loads the version and the profiles, then the workspaces of the selected (or first) profile.
	case load
	case selectProfile(path: String)
	/// Opens a tab with a login shell, outside any profile.
	case openShell
	/// Opens a tab on a workspace of the selected profile.
	case openWorkspace(Workspace)
	/// Opens a shell tab when there are no tabs, as at a launch that restored none.
	case openShellIfNoTabs
	case activateTab(id: UInt64)
	case closeTab(id: UInt64)
	/// Answers `StartupProblem.stateUnreadable`: moves the unreadable app state aside, keeping it, and opens a fresh one in its place.
	case startFresh
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
	/// Problems the core reported without failing, such as a corrupt `task.json`.
	public private(set) var warnings: [String] = []
	/// The last command's failure, cleared by the next success.
	public private(set) var error: String?
	/// The core-owned app state (tabs and their sessions), folded from the core's events.
	public private(set) var app = AppState.empty
	public private(set) var startupProblem: StartupProblem?

	/// Asks the window's display link to call `foldPending()` on its next frame; without one, events fold on the main actor's next turn.
	@ObservationIgnored public var requestFrame: (@MainActor () -> Void)?

	@ObservationIgnored private let configPath: String?
	@ObservationIgnored private var core: DevoraCore?
	@ObservationIgnored private let inbox = Inbox()
	@ObservationIgnored private var stateEvents: Task<Void, Never>?

	/// `configPath` overrides the global config file (`nil`: `DEVORA_CONFIG_PATH`, then `~/.config/devora/config.json`).
	public init(configPath: String?, state: StateStorage) {
		self.configPath = configPath
		open(state)
	}

	private func open(_ state: StateStorage) {
		do {
			let statePath: String? = switch state {
			case .inMemory: nil
			case .file(let path): path
			case .app: try sapphireStatePath(resourcesDir: Bundle.main.resourcePath)
			}
			let core = try DevoraCore(configPath: configPath, statePath: statePath)
			self.core = core
			stateEvents = pullStateEvents(from: try core.subscribe())
		} catch CoreError.StateInUse(let path, _) {
			startupProblem = .stateInUse(path: path)
		} catch CoreError.StateUnreadable(let path, let message) {
			startupProblem = .stateUnreadable(path: path, reason: message)
		} catch {
			self.error = Self.describe(error)
		}
	}

	isolated deinit {
		stateEvents?.cancel()
	}

	@discardableResult
	public func dispatch(_ command: Command) -> Task<Void, Never> {
		switch command {
		case .load, .selectProfile:
			return Task { await perform(command) }
		case .openShell:
			apply(Self.openShell)
		case .openShellIfNoTabs:
			guard let core else { break }
			do {
				if try core.snapshot().window.tabs.isEmpty { apply(Self.openShell) }
			} catch {
				self.error = Self.describe(error)
			}
		case .startFresh:
			startFresh()
		case .openWorkspace(let workspace):
			apply(.openSession(
				title: workspace.title.isEmpty ? workspace.id : workspace.title,
				kind: .workspace(path: workspace.path),
				profilePath: selectedProfilePath))
		case .activateTab(let id):
			apply(.activateTab(tab: id))
		case .closeTab(let id):
			apply(.closeSession(tab: id))
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
		guard let core else { throw CoreError.Failed(message: error ?? "the core failed to start") }
		let resources = Bundle.main.resourcePath
		let launch = try await Self.offMain { try core.terminalSpec(pane: id, resourcesDir: resources) }
		for warning in launch.warnings { NSLog("terminal %llu: %@", id, warning) }
		return launch.spec
	}

	private static let openShell = StateCommand.openSession(title: "Shell", kind: .shell, profilePath: nil)

	private func startFresh() {
		guard case .stateUnreadable(let path, _) = startupProblem else { return }
		do {
			let aside = try setAsideState(path: path)
			NSLog("Moved the unreadable app state aside to %@", aside)
		} catch {
			self.error = Self.describe(error)
			return
		}
		error = nil
		startupProblem = nil
		open(.file(path: path))
	}

	/// State commands apply in order and quickly (a reducer and one small SQLite transaction), so they run synchronously; their changes arrive through the subscription.
	private func apply(_ command: StateCommand) {
		guard let core else { return }
		do {
			try core.dispatch(command: command)
			error = nil
		} catch {
			self.error = Self.describe(error)
		}
	}

	private func pullStateEvents(from subscription: StateSubscription) -> Task<Void, Never> {
		let inbox = inbox
		return Task.detached { [weak self] in
			do {
				for try await batch in StateEvents(subscription) {
					if inbox.append(batch) { await self?.eventsArrived() }
				}
			} catch {
				await self?.fail(error)
			}
		}
	}

	private func eventsArrived() {
		if let requestFrame { requestFrame() } else { foldPending() }
	}

	private func fail(_ error: any Error) {
		self.error = Self.describe(error)
	}

	private func perform(_ command: Command) async {
		guard let core else { return }
		switch command {
		case .load:
			do {
				let (version, profiles) = try await Self.offMain { (try core.version(), try core.listProfiles()) }
				self.version = version
				self.profiles = profiles
				error = nil
				let selected = profiles.first { $0.path == selectedProfilePath } ?? profiles.first
				await select(selected?.path, core: core)
			} catch {
				self.error = Self.describe(error)
			}
		case .selectProfile(let path):
			await select(path, core: core)
		default:
			return
		}
	}

	private func select(_ path: String?, core: DevoraCore) async {
		selectedProfilePath = path
		guard let path else {
			workspaces = []
			warnings = []
			return
		}
		do {
			let list = try await Self.offMain { try core.listWorkspaces(profilePath: path) }
			// A later selection wins over a slower, earlier one.
			guard selectedProfilePath == path else { return }
			workspaces = list.workspaces
			warnings = list.warnings
			error = nil
		} catch {
			guard selectedProfilePath == path else { return }
			workspaces = []
			warnings = []
			self.error = Self.describe(error)
		}
	}

	/// Core calls do file I/O, so they run off the main actor.
	private static func offMain<T: Sendable>(_ body: @escaping @Sendable () throws -> T) async throws -> T {
		try await Task.detached(operation: body).value
	}

	public nonisolated static func describe(_ error: any Error) -> String {
		switch error as? CoreError {
		case .Failed(let message), .StateInUse(_, let message), .StateUnreadable(_, let message): message
		case nil: String(describing: error)
		}
	}
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
