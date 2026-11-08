import CoreBindings
import Foundation
import Observation

extension ChatSubscription: BatchSubscription {}
extension AgentCreationSubscription: BatchSubscription {}

/// An open agent's status at a glance, as its tab shows it (ADR-012).
public enum AgentStatus: Equatable, Sendable {
	case working
	/// A permission question waits for the user.
	case waiting
	case idle
	case pausing
	case paused
	case failed

	/// What a chat's lifecycle says about its agent; a pending question outranks a running turn.
	init(phase: ChatPhase, turnRunning: Bool, questions: Int) {
		switch phase {
		case .opening: self = .working
		case .running: self = questions > 0 ? .waiting : turnRunning ? .working : .idle
		case .pausing: self = .pausing
		case .paused: self = .paused
		case .failed: self = .failed
		}
	}
}

/// The New Agent dialog's preview: the instructions creation would render, or why it would fail.
public enum AgentPreview: Equatable, Sendable {
	case text(String)
	case failure(String)
}

/// One Agent Space creation, from its start until it ends: its steps, then its outcome.
@MainActor
@Observable
public final class AgentCreationRun {
	public enum Outcome: Equatable, Sendable {
		case done(name: String)
		case failed(String)
		case cancelled
	}

	public private(set) var steps: [String] = []
	public private(set) var outcome: Outcome?
	@ObservationIgnored var id: UInt64?
	@ObservationIgnored var cancelRequested = false

	public init() {}

	public var isRunning: Bool { outcome == nil }

	func progressed(_ events: [AgentCreationEvent]) {
		for event in events {
			switch event {
			case .step(let label): steps.append(label)
			case .log: break
			case .done(let name, _): outcome = .done(name: name)
			case .failed(let message): outcome = .failed(message)
			case .cancelled: outcome = .cancelled
			}
		}
	}

	func end(_ outcome: Outcome) {
		if self.outcome == nil { self.outcome = outcome }
	}
}

/// The Agent Hub's calls (ADR-012): listing, creating, deleting and opening the selected profile's Agent Spaces.
extension Store {
	/// Lists the selected profile's Agent Spaces into `agentSpaces`.
	@discardableResult
	public func listAgentSpaces() async -> Bool {
		guard let core, let profile = selectedProfilePath else {
			agentSpaces = []
			return selectedProfilePath == nil
		}
		guard let spaces = try? await call("list_agent_spaces", { try core.listAgentSpaces(profilePath: profile) }) else { return false }
		if profile == selectedProfilePath { agentSpaces = spaces }
		return true
	}

	/// The template's fields for the creation form, as `form` would render them.
	public func agentCreationFields(_ form: AgentCreationForm) async -> [AgentField]? {
		guard let core, let profile = selectedProfilePath else { return nil }
		return try? await call("agent_creation_fields", {
			try core.agentCreationFields(profilePath: profile, form: form, launchInputs: [:])
		})
	}

	/// What the New Agent form's model and effort start with, and the efforts it offers.
	public func agentModelDefaults() async -> AgentModelDefaults? {
		guard let core, let profile = selectedProfilePath else { return nil }
		return try? await call("agent_model_defaults", { try core.agentModelDefaults(profilePath: profile) })
	}

	/// The instructions `form` would create, or why creation would fail; a failure is the form's to show, not a banner.
	public func agentCreationPreview(_ form: AgentCreationForm, profilePath: String? = nil) async -> AgentPreview? {
		guard let core, let profile = profilePath ?? selectedProfilePath else { return nil }
		do {
			return .text(try await Task.detached { try core.agentCreationPreview(profilePath: profile, form: form, launchInputs: [:]) }.value)
		} catch {
			return .failure(Self.describe(error))
		}
	}

	/// Saves a creation-form value to the global or the profile config, as the Settings Hub would.
	public func saveAgentField(scope: AgentConfigScope, key: String, value: String) async -> Bool {
		guard let core, let profile = selectedProfilePath else { return false }
		return (try? await call("save_agent_field", {
			try core.saveAgentField(profilePath: profile, scope: scope, key: key, value: value)
		})) != nil
	}

	/// Creates an Agent Space in the selected profile, following its progress into `run`.
	public func createAgentSpace(_ form: AgentCreationForm, into run: AgentCreationRun) async {
		guard let core, let profile = selectedProfilePath,
		      let creation = try? await call("create_agent_space", {
		      	try core.createAgentSpace(profilePath: profile, form: form, launchInputs: [:])
		      })
		else {
			run.end(.failed("the creation didn't start"))
			return
		}
		run.id = creation.id
		if run.cancelRequested { cancelAgentCreation(run) }
		let events = creation.events
		do {
			for try await batch in Batches(events) {
				run.progressed(batch)
				if !run.isRunning { break }
			}
		} catch {
			showError("the progress of create_agent_space stopped: \(Self.describe(error))")
		}
		try? events.close()
		run.end(.failed("the progress stopped before the end"))
	}

	/// Cancels a running creation; one asked for before the core has returned its id is held until it has.
	public func cancelAgentCreation(_ run: AgentCreationRun) {
		run.cancelRequested = true
		guard let core, let id = run.id else { return }
		try? call("cancel_agent_creation") { try core.cancelAgentCreation(id: id) }
	}

	/// Deletes an agent's space; an open agent is paused and its tab closed first, since the core deletes only a stopped one.
	public func deleteAgentSpace(_ name: String) async -> Bool {
		guard let core, let profile = selectedProfilePath else { return false }
		do {
			// The core records the profile's canonical path, which may differ from the configured one (/var and /private/var).
			let canonical = { (path: String) in URL(fileURLWithPath: path).resolvingSymlinksInPath().path }
			if let tab = app.window.tabs.first(where: {
				$0.session.kind == .agentSpace(name: name) && $0.session.profilePath.map(canonical) == canonical(profile)
			}) {
				guard agentModels[tab.id]?.value?.flushDraft() != false else { return false }
				try await core.pauseAgentSpace(tab: tab.id)
				try core.closeAgentSpace(tab: tab.id)
			}
			try await core.deleteAgentSpace(profilePath: profile, name: name)
		} catch {
			_ = shown(error, of: "delete_agent_space")
			return false
		}
		await listAgentSpaces()
		return true
	}

	/// Opens an agent in its tab, or activates the tab it already has; the launch comes from the user's login shell.
	public func openAgentSpace(_ name: String) async {
		guard let core, let profile = selectedProfilePath, let launch = await agentLaunch(core) else { return }
		do {
			_ = try await core.openAgentSpace(profilePath: profile, name: name, launch: launch)
		} catch {
			_ = shown(error, of: "open_agent_space")
		}
	}

	/// Resumes the agents of the tabs a launch restored; each failure shows on its own tab.
	public func restoreAgentSpaces() async {
		guard let core, hasAgentTabs(core), let launch = await agentLaunch(core)
		else { return }
		do {
			try await core.restoreAgentSpaces(launch: launch)
		} catch {
			_ = shown(error, of: "restore_agent_spaces")
		}
	}

	/// Pauses every agent, waiting until each has stopped and saved its session; it calls the core directly, so it can run while the main actor is busy, as it is during a quit.
	public func agentShutdown() -> (@Sendable () async -> Void)? {
		guard let core else { return nil }
		return { try? await core.shutdownAgents() }
	}

	/// The restored tabs fold on a later frame, so the launch reads them from the core.
	private func hasAgentTabs(_ core: DevoraCore) -> Bool {
		guard let snapshot = try? core.snapshot() else { return false }
		return snapshot.window.tabs.contains { if case .agentSpace = $0.session.kind { true } else { false } }
	}

	private func agentLaunch(_ core: DevoraCore) async -> AgentLaunch? {
		do {
			return try await core.resolveAgentLaunch(resourcesDir: Bundle.main.resourcePath)
		} catch {
			_ = shown(error, of: "resolve_agent_launch")
			return nil
		}
	}

	/// Folds the chats' lifecycle into `agentStatuses` off the main actor, hopping to it only when a status changes, since the chats also carry every streamed token.
	func pullAgentStatuses(from subscription: ChatSubscription) -> Task<Void, Never> {
		Task.detached { [weak self] in
			var chats: [UInt64: (phase: ChatPhase, turn: Bool, questions: Set<String>, pause: Bool)] = [:]
			var published: [UInt64: AgentStatus] = [:]
			do {
				for try await batch in Batches(subscription) {
					for event in batch {
						switch event {
						case .snapshot(let all):
							chats = Dictionary(uniqueKeysWithValues: all.map {
								($0.tab, ($0.phase, $0.turnRunning, Set($0.questions.map(\.requestId)), $0.pauseRequested))
							})
						case .started(let chat):
							chats[chat.tab] = (chat.phase, chat.turnRunning, [], chat.pauseRequested)
						case .closed(let tab):
							chats[tab] = nil
						case .updated(let tab, _, let update):
							guard var chat = chats[tab] else { continue }
							switch update {
							case .turn(let running): chat.turn = running
							case .pauseRequested(let requested): chat.pause = requested
							case .generation: chat.questions = []
							case .phase(let phase):
								chat.phase = phase
								if phase == .paused || phase == .failed { chat.pause = false }
								if phase != .running { chat.questions = []; chat.turn = false }
							case .ready:
								chat.phase = .running
								chat.turn = false
								chat.questions = []
							case .permissionRequested(let question): chat.questions.insert(question.requestId)
							case .permissionRemoved(let id): chat.questions.remove(id)
							case .failure:
								chat.phase = .failed
								chat.pause = false
								chat.turn = false
								chat.questions = []
							case .restore, .transcript, .usage, .context, .globalHold: continue
							}
							chats[tab] = chat
						}
					}
					let statuses = chats.mapValues { $0.pause ? .pausing : AgentStatus(phase: $0.phase, turnRunning: $0.turn, questions: $0.questions.count) }
					guard statuses != published else { continue }
					published = statuses
					await MainActor.run { [weak self] in self?.agentStatuses = statuses }
				}
			} catch {
				await MainActor.run { [weak self] in
					self?.showError("the agents' events stopped: \(Self.describe(error))")
				}
			}
		}
	}
}
