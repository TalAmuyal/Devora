import CoreBindings
import Foundation
import Testing

@testable import Store

@MainActor
struct GlobalPauseTests {
	@Test func globalPauseFoldsAndKeepsARefusedPromptInTheDraft() async throws {
		let fixture = try Fixture()
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		let core = try #require(store.core)
		try core.dispatch(command: .openSession(title: "Draft", kind: .shell, profilePath: "/profile"))
		let tab = try #require(core.snapshot().window.tabs.first)
		let model = try AgentChat(store: store, core: core, tab: tab, name: "Draft")
		model.apply(.started(chat: Chat(tab: tab.id, generation: 1, phase: .running, turnRunning: false, pauseRequested: false, globallyHeld: false, sessionId: "cli", items: [], error: nil, questions: [], usageJson: nil, context: nil)))
		model.draft = "Keep this prompt"
		await store.dispatch(.setGlobalPaused(true)).value
		try await eventually { store.app.globalPaused }
		#expect(model.globallyPaused)
		#expect(!model.canSend)
		await model.send()
		#expect(model.draft == "Keep this prompt")
		#expect(model.flushDraft())
		#expect(try core.snapshot().agentViews.first?.draft == "Keep this prompt")
		await store.dispatch(.setGlobalPaused(false)).value
		try await eventually { !store.app.globalPaused }
		#expect(model.canSend)
		#expect(model.draft == "Keep this prompt")
	}
}
