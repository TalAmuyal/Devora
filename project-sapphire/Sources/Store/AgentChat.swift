import CoreBindings
import Foundation
import Observation

/// A tab's chat projection. Older pages are explicitly loaded by the reader.
/// The core owns durable drafts/anchors and validates generation-scoped controls.
@MainActor
@Observable
public final class AgentChat {
	public let tab: UInt64
	public let profile: String
	public let name: String
	public private(set) var chat: Chat?
	public private(set) var items: [TranscriptItem] = []
	public var draft: String { didSet { if !acceptingDraft { saveDraftLater() } } }
	public private(set) var anchor: ChatScrollAnchor?
	public private(set) var hasOlder = true
	public private(set) var loadingOlder = false
	public private(set) var sending = false
	public private(set) var revision: UInt64 = 0
	public private(set) var enterSends = false
	public private(set) var error: String?
	@ObservationIgnored private weak var store: Store?
	@ObservationIgnored private let core: DevoraCore
	@ObservationIgnored private var events: Task<Void, Never>?
	@ObservationIgnored private var draftSave: Task<Void, Never>?
	@ObservationIgnored private var initializing = true
	@ObservationIgnored private var acceptingDraft = false

	init(store: Store, core: DevoraCore, tab: AppTab, name: String) throws {
		self.store = store
		self.core = core
		self.tab = tab.id
		self.name = name
		let profile = tab.session.profilePath ?? ""
		self.profile = profile
		let saved = try core.snapshot().agentViews.first { $0.profilePath == profile && $0.name == name }
		draft = saved?.draft ?? ""
		anchor = saved?.anchor
		let subscription = try core.subscribeAgentChats()
		events = Task { [weak self] in
			do {
				for try await batch in Batches(subscription) {
					guard let self else { break }
					var changed = false
					for event in batch { if self.apply(event) { changed = true } }
					guard changed else { continue }
					if self.anchor == nil && self.items.count > 200 {
						self.items.removeFirst(self.items.count - 200)
						self.hasOlder = true
					}
					self.revision &+= 1
				}
			} catch { self?.fail(error) }
		}
		Task { [weak self] in await self?.refreshSettings() }
	}

	isolated deinit {
		events?.cancel()
		draftSave?.cancel()
	}

	public func refreshSettings() async {
		guard let settings = await store?.configSettings(profilePath: profile) else { return }
		if case .bool(let value)? = settings.resolved["agent.enter-sends"] { enterSends = value }
	}

	public var globallyPaused: Bool { store?.app.globalPaused == true }

	public var canSend: Bool {
		!globallyPaused &&
		chat?.phase == .running && chat?.pauseRequested == false && chat?.turnRunning == false && !sending
			&& !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
	}

	@discardableResult
	func apply(_ event: ChatEvent) -> Bool {
		switch event {
		case .snapshot(let chats):
			guard let value = chats.first(where: { $0.tab == tab }) else { return false }
			restore(value)
		case .started(let value) where value.tab == tab: restore(value)
		case .updated(let id, let generation, let update) where id == tab:
			guard var value = chat, value.generation == generation else { return false }
			switch update {
			case .pauseRequested(let requested): value.pauseRequested = requested
			case .globalHold(let held): value.globallyHeld = held
			case .generation(let next):
				value.generation = next
				value.questions = []
			case .turn(let running): value.turnRunning = running
			case .phase(let phase):
				value.phase = phase
				if phase != .running {
					value.questions = []
					value.turnRunning = false
				}
				if phase == .paused || phase == .failed { value.pauseRequested = false }
			case .ready(let session):
				value.phase = .running
				value.sessionId = session
				value.error = nil
				value.turnRunning = false
				value.questions = []
			case .failure(let message):
				value.phase = .failed
				value.error = message
				value.questions = []
				value.turnRunning = false
				value.pauseRequested = false
			case .restore(let restored): items = restored
			case .transcript(let change): apply(change)
			case .permissionRequested(let question):
				value.questions.removeAll { $0.requestId == question.requestId }
				value.questions.append(question)
			case .permissionRemoved(let id): value.questions.removeAll { $0.requestId == id }
			case .usage(let json): value.usageJson = json
			case .context(let context): value.context = context
			}
			chat = value
		default: return false
		}
		return true
	}

	private func restore(_ value: Chat) {
		chat = value
		items = value.items
		hasOlder = true
	}

	func apply(_ change: TranscriptChange) {
		switch change {
		case .upsert(let item):
			if let index = items.firstIndex(where: { $0.sequence == item.sequence }) {
				items[index] = item
			} else {
				items.append(item)
				items.sort { $0.sequence < $1.sequence }
			}
		case .appendText(let sequence, let text):
			guard let index = items.firstIndex(where: { $0.sequence == sequence }) else { return }
			switch items[index].content {
			case .assistant(let old): items[index].content = .assistant(text: old + text)
			default: break
			}
		case .appendThinking(let sequence, let text):
			guard let index = items.firstIndex(where: { $0.sequence == sequence }),
				case .thinking(let old, let signature) = items[index].content
			else { return }
			items[index].content = .thinking(text: old + text, signature: signature)
		case .appendSignature(let sequence, let signature):
			guard let index = items.firstIndex(where: { $0.sequence == sequence }),
				case .thinking(let text, let old) = items[index].content
			else { return }
			items[index].content = .thinking(text: text, signature: old + signature)
		case .appendToolInput(let sequence, let json):
			guard let index = items.firstIndex(where: { $0.sequence == sequence }),
				case .tool(let id, let name, let input, let verdict, let result) = items[index].content
			else { return }
			items[index].content = .tool(
				toolUseId: id, name: name, inputJson: (input == "{}" ? "" : input) + json,
				verdict: verdict, result: result)
		}
	}

	/// Load enough older pages to find the saved top row before first rendering.
	public func prepareHistory() async {
		guard initializing else { return }
		if let anchor {
			while let first = items.first, first.sequence > anchor.sequence && hasOlder {
				await older()
				if error != nil { break }
			}
		}
		initializing = false
	}

	public func older() async {
		guard !loadingOlder, hasOlder, let first = items.first else { return }
		loadingOlder = true
		defer {
			loadingOlder = false
			revision &+= 1
		}
		do {
			let page = try await core.agentHistoryBefore(
				profilePath: profile, name: name, before: first.sequence, limit: 200)
			let existing = Set(items.map(\.sequence))
			items.insert(contentsOf: page.filter { !existing.contains($0.sequence) }, at: 0)
			hasOlder = page.count == 200
		} catch { fail(error) }
	}

	public func saveScroll(_ value: ChatScrollAnchor?) {
		anchor = value
		do {
			_ = try core.dispatch(command: .setAgentScroll(profilePath: profile, name: name, anchor: value))
		} catch { fail(error) }
	}

	private func saveDraftLater() {
		draftSave?.cancel()
		draftSave = Task { [weak self] in
			do { try await Task.sleep(for: .milliseconds(250)) } catch { return }
			self?.flushDraft()
		}
	}

	@discardableResult
	public func flushDraft() -> Bool {
		draftSave?.cancel()
		do {
			_ = try core.dispatch(command: .setAgentDraft(profilePath: profile, name: name, text: draft))
			return true
		} catch {
			fail(error)
			return false
		}
	}

	func acceptedDraft(_ text: String) {
		guard draft == text else { return }
		draftSave?.cancel()
		acceptingDraft = true
		draft = ""
		acceptingDraft = false
	}

	public func send() async {
		guard canSend, let generation = chat?.generation, flushDraft() else { return }
		let text = draft
		sending = true
		defer { sending = false }
		do {
			try await core.sendAgentTurn(tab: tab, generation: generation, text: text)
			acceptedDraft(text)
		} catch { fail(error) }
	}

	public func stop() async {
		guard let generation = chat?.generation else { return }
		do { try await core.interruptAgent(tab: tab, generation: generation) } catch { fail(error) }
	}

	public func pause() {
		guard let generation = chat?.generation else { return }
		do { try core.pauseAgentAfterTurn(tab: tab, generation: generation) } catch { fail(error) }
	}

	public func retry() async {
		do {
			let launch = try await core.resolveAgentLaunch(resourcesDir: Bundle.main.resourcePath)
			_ = try await core.openAgentSpace(profilePath: profile, name: name, launch: launch)
			error = nil
		} catch { fail(error) }
	}

	public func answer(_ id: String, action: String, generation: UInt64) async {
		guard let chat, chat.generation == generation,
			let question = chat.questions.first(where: { $0.requestId == id })
		else { return }
		let answer: PermissionAnswer
		switch action {
		case "allow": answer = .allow
		case "deny": answer = .deny(message: "Denied by the user")
		case "always" where question.canAlwaysAllow: answer = .allowWithSuggestedRule
		default: return
		}
		do {
			try await core.answerAgentPermission(
				tab: tab, generation: chat.generation, requestId: id, answer: answer)
		} catch { fail(error) }
	}

	private func fail(_ error: any Error) {
		self.error = Store.describe(error)
		store?.showError("agent chat failed: \(Store.describe(error))")
	}
}

/// Store observes models without owning their view lifetime.
@MainActor
final class WeakAgentChat {
	weak var value: AgentChat?
	init(_ value: AgentChat) { self.value = value }
}

extension Store {
	public func flushAgentDrafts() -> Bool {
		var saved = true
		for model in agentModels.values { if model.value?.flushDraft() == false { saved = false } }
		return saved
	}

	public func forgetAgentChat(tab: UInt64) { agentModels[tab] = nil }

	public func agentChat(tab id: UInt64) -> AgentChat? {
		if let model = agentModels[id]?.value { return model }
		guard let core, let tab = app.window.tabs.first(where: { $0.id == id }),
			case .agentSpace(let name) = tab.session.kind
		else { return nil }
		do {
			let model = try AgentChat(store: self, core: core, tab: tab, name: name)
			agentModels[id] = WeakAgentChat(model)
			return model
		} catch {
			_ = shown(error, of: "agent_chat")
			return nil
		}
	}
}
