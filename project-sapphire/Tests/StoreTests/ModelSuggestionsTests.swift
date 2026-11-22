import CoreBindings
import Store
import Testing

/// A subscription's list as the core hands it over (Claude Code 2.1.312's): an agent's choices and a tier's, each in the
/// core's order, which the core's own tests cover (`devora_agent::models`).
private let subscription = ModelSuggestions(AgentModelList(
	cliVersion: "2.1.312",
	models: [
		option("opus", "claude-opus-5-5", "Opus 5.5"),
		option("sonnet", "claude-sonnet-5-5", "Sonnet 5.5"),
		option("haiku", "claude-haiku-5-5", "Haiku 5.5"),
		option("claude-opus-4-8", "claude-opus-4-8", "Opus 4.8"),
		option("claude-sonnet-4-6", "claude-sonnet-4-6", "Sonnet 4.6"),
		option("claude-haiku-4-5-20251001", "claude-haiku-4-5-20251001", "Haiku 4.5"),
	],
	tierModels: [
		TierModel(id: "claude-opus-5-5", name: "Opus 5.5"), TierModel(id: "claude-sonnet-5-5", name: "Sonnet 5.5"),
		TierModel(id: "claude-opus-4-8", name: "Opus 4.8"),
	]))

private func option(_ value: String, _ resolved: String?, _ name: String) -> AgentModelOption {
	AgentModelOption(value: value, resolvedModel: resolved, displayName: name, description: nil, effortLevels: [])
}

struct ModelSuggestionsTests {
	@Test func anAgentsRowsKeepTheCoresOrderAndAPickStoresTheAlias() {
		let rows = subscription.rows(for: .agentModel)
		#expect(rows.map(\.completion) == [
			"opus", "sonnet", "haiku", "claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5-20251001",
		])
		let opus = rows[0]
		#expect((opus.name, opus.alias, opus.modelID) == ("Opus 5.5", "opus", "claude-opus-5-5"))
		#expect(rows[3].alias == nil, "A full ID is stored as itself")
	}

	@Test func aTiersRowsAreTheCoresFullIDs() {
		let rows = subscription.rows(for: .tier)
		#expect(rows.map(\.completion) == ["claude-opus-5-5", "claude-sonnet-5-5", "claude-opus-4-8"])
		#expect((rows[0].name, rows[0].alias) == ("Opus 5.5", nil))
	}

	@Test func theMenuNarrowsToWhatIsTypedUnlessItIsAlreadyAPick() {
		#expect(subscription.rows(for: .agentModel, typed: "son").map(\.completion) == ["sonnet", "claude-sonnet-4-6"])
		#expect(subscription.rows(for: .agentModel, typed: "4.8").map(\.completion) == ["claude-opus-4-8"], "By name too")
		#expect(subscription.rows(for: .agentModel, typed: "nothing-like-it").isEmpty)
		#expect(subscription.rows(for: .agentModel, typed: "opus").count == 6, "A picked value shows every alternative")
	}

	@Test func theNoteNeverCallsAValueWrong() {
		#expect(subscription.note(for: "sonnet") == "Suggestions from Claude Code 2.1.312. Any model name works.")
		#expect(subscription.note(for: "claude-opus-5-5") == "Suggestions from Claude Code 2.1.312. Any model name works.")
		#expect(subscription.note(for: "opusplan") == "Opus while planning, Sonnet for the rest.")
		#expect(subscription.note(for: "my-proxy-model") == "Any model name works.")
		#expect(ModelSuggestions.note(for: "opusplan", listedBy: nil) == "Opus while planning, Sonnet for the rest.")
		#expect(ModelSuggestions.note(for: "sonnet", listedBy: nil) == "Any model name works.")
		#expect(subscription.note(for: " ") == "Required: pick a model or type its name.")
	}
}

@MainActor
struct ModelLookupInTestsTests {
	/// A test that names no fake CLI mustn't launch the user's own `claude`: the lookup doesn't run at all. Had it run, it
	/// would have answered a list or logged why it couldn't.
	@Test func aTestRunNamingNoClaudeProgramLaunchesNothing() async throws {
		let fixture = try Fixture()
		let profile = try fixture.makeProfile("Work")
		try fixture.registerProfiles([profile])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.load(selecting: profile)
		#expect(await store.modelSuggestions(environment: ["DEVORA_TEST_MODE": "1"]) == nil)
		#expect(try fixture.logLines().filter { $0.contains("model suggestions") }.isEmpty, "Nothing was tried")
	}
}
