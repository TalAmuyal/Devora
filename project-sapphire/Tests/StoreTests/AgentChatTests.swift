import CoreBindings
import Foundation
import Testing

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
			tab: tab.id, generation: 7, phase: .running, turnRunning: false, pauseRequested: false, globallyHeld: false,
			sessionId: "cli", items: [], error: nil, questions: [], usageJson: nil, context: nil)
		model.apply(.started(chat: chat))
		let item = TranscriptItem(
			sequence: 1, cliSession: "cli", parentToolUseId: nil, state: .streaming,
			content: .assistant(text: "Hel"))
		model.apply(.updated(tab: tab.id, generation: 7, update: .transcript(change: .upsert(item: item))))
		model.apply(
			.updated(
				tab: tab.id, generation: 7,
				update: .transcript(change: .appendText(sequence: 1, text: "lo"))))
		#expect(model.items.count == 1)
		#expect(model.items[0].content == .assistant(text: "Hello"))
		model.apply(.updated(tab: tab.id, generation: 6, update: .failure(message: "stale")))
		#expect(model.chat?.phase == .running)
		let context = ChatContext(usedTokens: 2500, windowTokens: 200000, thresholdPercent: 75)
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
}
