import CoreBindings
import Store
import Testing

/// A subscription's list, as Claude Code 2.1.312 gave it: its aliases, then other models' full IDs, older ones included.
private let subscription = ModelSuggestions(AgentModelList(cliVersion: "2.1.312", models: [
	option("default", "claude-opus-5-5", "Default (recommended)"),
	option("opus", "claude-opus-5-5", "Opus 5.5"),
	option("sonnet", "claude-sonnet-5-5", "Sonnet 5.5"),
	option("haiku", "claude-haiku-5-5", "Haiku 5.5"),
	option("claude-haiku-4-5-20251001", "claude-haiku-4-5-20251001", "Haiku 4.5"),
	option("claude-opus-4-8", "claude-opus-4-8", "Opus 4.8"),
	option("claude-sonnet-4-6", "claude-sonnet-4-6", "Sonnet 4.6"),
]))

private func option(_ value: String, _ resolved: String?, _ name: String) -> AgentModelOption {
	AgentModelOption(value: value, resolvedModel: resolved, displayName: name, description: nil, effortLevels: [])
}

struct ModelSuggestionsTests {
	@Test func anAgentsModelIsOfferedEveryModelButClaudeCodesDefaultAndAPickStoresTheAlias() {
		let rows = subscription.rows(for: .agentModel)
		#expect(rows.map(\.completion) == [
			"opus", "sonnet", "haiku", "claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5-20251001",
		])
		let opus = rows[0]
		#expect((opus.name, opus.alias, opus.modelID) == ("Opus 5.5", "opus", "claude-opus-5-5"))
		#expect(rows[3].alias == nil, "A full ID is stored as itself")
	}

	@Test func aTierIsOfferedEachFullIDOnce() {
		#expect(subscription.rows(for: .tier).map(\.completion) == [
			"claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-opus-4-8", "claude-sonnet-4-6",
			"claude-haiku-4-5-20251001",
		])
		#expect(subscription.rows(for: .tier)[0].name == "Opus 5.5", "An alias's model is named by its alias, not by `default`")
	}

	@Test func modelsReadNewestGenerationFirstThenByTierThenTheRestByID() {
		let ids = [
			"claude-haiku-4-5-20251001", "zeta-model", "claude-opus-5", "claude-sonnet-5-5", "claude-opus-5-2", "claude-fable-5-5",
			"claude-mystery-6-0", "claude-opus-4-8", "claude-opus", "alpha-model", "claude-haiku-5-5", "claude-opus-5-5",
		]
		#expect(ids.sorted(by: ModelSuggestions.precedes) == [
			"claude-fable-5-5", "claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-opus-5-2", "claude-opus-5",
			"claude-opus-4-8", "claude-haiku-4-5-20251001",
			// Outside the scheme, in plain order: an unknown family, no number at all, another naming.
			"alpha-model", "claude-mystery-6-0", "claude-opus", "zeta-model",
		])
	}

	@Test func aModelWithoutANameIsNamedByItsID() {
		let unnamed = ModelSuggestions(AgentModelList(cliVersion: nil, models: [option("claude-next", "claude-next", "")]))
		#expect(unnamed.rows(for: .agentModel).map(\.name) == ["claude-next"])
		#expect(unnamed.rows(for: .tier).map(\.name) == ["claude-next"])
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
