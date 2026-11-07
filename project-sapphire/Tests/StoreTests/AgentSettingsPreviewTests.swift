import CoreBindings
import Foundation
import Store
import Testing

@MainActor
struct AgentSettingsPreviewTests {
	@Test func previewUsesTheOwningProfileAndReportsCreationErrorsInline() async throws {
		let fixture = try Fixture()
		let active = try fixture.makeProfile("Active")
		let other = try fixture.makeProfile("Other")
		try fixture.registerProfiles([active, other])
		let file = URL(filePath: other).appending(path: "template.md")
		try "Profile prompt {{agent.role}} in {{space.path}}".write(to: file, atomically: true, encoding: .utf8)
		try #"{"name":"Other","agent":{"template-file":"template.md","role":"Other role"}}"#
			.write(to: URL(filePath: other).appending(path: "config.json"), atomically: true, encoding: .utf8)
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.load(selecting: active)
		let form = AgentCreationForm(name: "", role: nil, repos: [], inputs: [:], keepCodingInstructions: true)
		let preview = await store.agentCreationPreview(form, profilePath: other)
		guard case .text(let text) = preview else {
			Issue.record("Expected the inactive profile's saved template")
			return
		}
		#expect(text.contains("Other role"))
		#expect(text.contains(other + "/agent-spaces/<name>"))
		#expect(store.selectedProfilePath == active)
		try "Missing {{required.value}}".write(to: file, atomically: true, encoding: .utf8)
		guard case .failure(let reason) = await store.agentCreationPreview(form, profilePath: other) else {
			Issue.record("Expected a creation-error preview")
			return
		}
		#expect(reason.contains("required.value"))
		#expect(store.scrapeErrors().isEmpty)
		#expect(!FileManager.default.fileExists(atPath: other + "/agent-spaces"))
	}
}
