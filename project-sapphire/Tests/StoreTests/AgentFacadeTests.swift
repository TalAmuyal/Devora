import CoreBindings
import Foundation
import Testing

@Test func agentFacadeBindingsCreateListAndDeleteWithoutOpeningAProcess() async throws {
	let fixture = try Fixture()
	let profile = try fixture.makeProfile("agents")
	let core = try DevoraCore(configPath: fixture.configPath, statePath: nil, log: fixture.log())
	let chatEvents = try core.subscribeAgentChats()
	let snapshot = try await chatEvents.nextBatch(max: 100)
	guard case .snapshot(let chats) = snapshot.first else {
		Issue.record("Expected an initial chat snapshot")
		return
	}
	#expect(chats.isEmpty)
	try chatEvents.close()
	#expect(try await chatEvents.nextBatch(max: 100).isEmpty)

	let creation = try core.createAgentSpace(
		profilePath: profile,
		form: AgentCreationForm(name: "Delta", role: "Developer", repos: [], inputs: [:]),
		launchInputs: [:])
	var finished = false
	while !finished {
		let batch = try await creation.events.nextBatch(max: 100)
		#expect(!batch.isEmpty)
		for event in batch {
			switch event {
			case .done(let name, _):
				#expect(name == "Delta")
				finished = true
			case .failed(let message):
				Issue.record("Agent creation failed \(message)")
				return
			case .cancelled:
				Issue.record("Agent creation was cancelled")
				return
			case .step, .log:
				break
			}
		}
	}
	try creation.events.close()
	let spaces = try core.listAgentSpaces(profilePath: profile)
	#expect(spaces.count == 1)
	#expect(spaces.first?.role == "Developer")
	#expect(spaces.first?.error == nil)
	try await core.deleteAgentSpace(profilePath: profile, name: "Delta")
	#expect(try core.listAgentSpaces(profilePath: profile).isEmpty)
	try await core.shutdownAgents()
}
