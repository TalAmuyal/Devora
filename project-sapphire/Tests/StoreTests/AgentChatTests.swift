import CoreBindings
import Foundation
import Testing
import TestSupport

@testable import Store

@MainActor
struct AgentChatTests {
	@Test func draftAndAnchorAreCoreStateAcrossClosingAndReopeningATab() throws {
		let fixture = try Fixture()
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		let core = try #require(store.core)
		_ = try core.dispatch(command: .openSession(title: "Delta", kind: .shell, profilePath: "/profile"))
		let tab = try #require(core.snapshot().window.tabs.first)
		var model: AgentChat? = try AgentChat(store: store, core: core, tab: tab, name: "Delta")
		model!.draft = "unsent\nsecond line"
		#expect(model!.flushDraft())
		model!.saveScroll(ChatScrollAnchor(sequence: 15, offset: -9))
		_ = try core.dispatch(command: .closeSession(tab: tab.id))
		model = nil
		_ = try core.dispatch(command: .openSession(title: "Delta", kind: .shell, profilePath: "/profile"))
		let reopened = try #require(core.snapshot().window.tabs.first)
		let restored = try AgentChat(store: store, core: core, tab: reopened, name: "Delta")
		#expect(restored.draft == "unsent\nsecond line")
		#expect(restored.anchor == ChatScrollAnchor(sequence: 15, offset: -9))
	}

	@Test func streamedItemsStayOnTheirSequenceAndStaleGenerationsAreIgnored() throws {
		let fixture = try Fixture()
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		let core = try #require(store.core)
		_ = try core.dispatch(command: .openSession(title: "Delta", kind: .shell, profilePath: "/profile"))
		let tab = try #require(core.snapshot().window.tabs.first)
		let model = try AgentChat(store: store, core: core, tab: tab, name: "Delta")
		let chat = Chat(
			tab: tab.id, generation: 7, phase: .running, turnRunning: false, agentTurn: false, pauseRequested: false, globallyHeld: false, keepPaused: false,
			sessionId: "cli", items: [], error: nil, questions: [], usageJson: nil, context: nil, model: nil, effort: nil, step: nil)
		model.apply(.started(chat: chat))
		let item = TranscriptItem(
			sequence: 1, cliSession: "cli", parentToolUseId: nil, state: .streaming,
			content: .assistant(text: "Hel", model: nil), createdAtMs: nil)
		model.apply(.updated(tab: tab.id, generation: 7, update: .transcript(change: .upsert(item: item))))
		model.apply(
			.updated(
				tab: tab.id, generation: 7,
				update: .transcript(change: .appendText(sequence: 1, text: "lo"))))
		#expect(model.items.count == 1)
		#expect(model.items[0].content == .assistant(text: "Hello", model: nil))
		model.apply(.updated(tab: tab.id, generation: 6, update: .failure(message: "stale")))
		#expect(model.chat?.phase == .running)
		let context = ChatContext(usedTokens: 2500, windowTokens: 200000, thresholdPercent: 75, defaultThresholdPercent: 75, ownThresholdPercent: nil, compaction: .idle)
		model.apply(.updated(tab: tab.id, generation: 7, update: .context(context: context)))
		#expect(model.chat?.context == context)
		model.draft = "new turn"
		#expect(model.canSend)
		model.apply(.updated(tab: tab.id, generation: 7, update: .pauseRequested(requested: true)))
		#expect(!model.canSend)
		model.apply(.updated(tab: tab.id, generation: 7, update: .failure(message: "failed")))
		#expect(model.chat?.phase == .failed)
		#expect(model.chat?.pauseRequested == false)
	}

	/// The chat folds turns by the core's rule: the agent's own turn keeps Send, a prompt sent into it makes the turn the
	/// user's, and a late end of the agent's turn never ends the user's, so no spinner is left on.
	@Test func theAgentsOwnTurnKeepsSendAndNeverEndsTheUsers() throws {
		let fixture = try Fixture()
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		let core = try #require(store.core)
		_ = try core.dispatch(command: .openSession(title: "Delta", kind: .shell, profilePath: "/profile"))
		let tab = try #require(core.snapshot().window.tabs.first)
		let model = try AgentChat(store: store, core: core, tab: tab, name: "Delta")
		model.apply(.started(chat: Chat(
			tab: tab.id, generation: 1, phase: .running, turnRunning: false, agentTurn: false, pauseRequested: false,
			globallyHeld: false, keepPaused: false, sessionId: "cli", items: [], error: nil, questions: [], usageJson: nil,
			context: nil, model: nil, effort: nil, step: nil)))
		let turn = { (running: Bool, byAgent: Bool) in
			model.apply(.updated(tab: tab.id, generation: 1, update: .turn(running: running, byAgent: byAgent)))
		}
		let shown = { (model.chat?.turnRunning, model.chat?.agentTurn) }
		model.draft = "a prompt"

		turn(true, true)
		#expect(shown() == (true, true))
		#expect(model.canSend, "a prompt joins the agent's own turn")
		turn(true, false)
		#expect(shown() == (true, false))
		#expect(!model.canSend, "the user's turn takes no second prompt")
		turn(false, true)
		#expect(shown() == (true, false), "a late end of the agent's turn")
		turn(false, false)
		#expect(shown() == (false, false))

		turn(true, false)
		turn(true, true)
		#expect(shown() == (true, false), "the agent's turn starting just after the user sent")
		turn(false, false)
		#expect(shown() == (false, false), "no spinner is left on")
	}

	/// The core's word that a sent draft was cleared can fold after the user has typed the same text again (it comes
	/// through the app state, which folds on a later frame than the chat); that echo of the chat's own send must not empty
	/// the new draft.
	@Test func theEchoOfASendLeavesTheSameTextTypedAgainAfterIt() async throws {
		let fixture = try Fixture()
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		let core = try #require(store.core)
		_ = try core.dispatch(command: .openSession(title: "Delta", kind: .shell, profilePath: "/profile"))
		store.foldPending()
		let tab = try #require(core.snapshot().window.tabs.first)
		let model = try AgentChat(store: store, core: core, tab: tab, name: "Delta")
		store.agentModels[tab.id] = WeakAgentChat(model)
		model.draft = "continue"
		#expect(model.flushDraft())
		// What a successful send does: the core clears the stored draft (its event waits for the next fold), and the chat
		// clears its own.
		_ = try core.dispatch(command: .clearAgentDraft(profilePath: model.profile, name: model.name, text: "continue"))
		model.accepted(sent: "continue")
		#expect(model.draft == "")
		model.draft = "continue"
		#expect(model.flushDraft())
		// The stored draft's change comes after the clear on the same stream: once it's folded, so is the clear's echo.
		try await eventually {
			store.app.agentViews.first { $0.profilePath == model.profile && $0.name == model.name }?.draft == "continue"
		}
		#expect(model.draft == "continue", "the text typed again stays")

		// Two sends of the same text (into the agent's own turn, Send stays) give two echoes, and both are this chat's.
		for _ in 0..<2 {
			model.draft = "yes"
			#expect(model.flushDraft())
			_ = try core.dispatch(command: .clearAgentDraft(profilePath: model.profile, name: model.name, text: "yes"))
			model.accepted(sent: "yes")
		}
		model.draft = "yes"
		#expect(model.flushDraft())
		try await eventually {
			store.app.agentViews.first { $0.profilePath == model.profile && $0.name == model.name }?.draft == "yes"
		}
		#expect(model.draft == "yes", "the text typed after both sends stays")

		// A clear this chat didn't send (another view of the agent sent it) still empties a draft of that text.
		model.draft = "continue"
		#expect(model.flushDraft())
		_ = try core.dispatch(command: .clearAgentDraft(profilePath: model.profile, name: model.name, text: "continue"))
		try await eventually { model.draft == "" }
	}
}
