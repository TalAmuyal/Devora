import CoreBindings
import Store
import Testing

/// A list as the core hands it over (its own tests cover how it's built: `devora_agent::models`): the tiers, then every
/// model by its version. The Haiku tier points at Sonnet here, as a profile can set it.
private let subscription = ModelSuggestions(AgentModelList(
	cliVersion: "2.1.329",
	aliases: [
		row("fable", "Fable (currently 5.2)", "claude-fable-5-2"), row("opus", "Opus (currently 5.5)", "claude-opus-5-5"),
		row("sonnet", "Sonnet (currently 5.5)", "claude-sonnet-5-5"),
		row("haiku", "Haiku (currently Sonnet 5.5)", "claude-sonnet-5-5"),
	],
	modelIds: [
		row("claude-fable-5-2", "Fable 5.2", "claude-fable-5-2"), row("claude-opus-5-5", "Opus 5.5", "claude-opus-5-5"),
		row("claude-sonnet-5-5", "Sonnet 5.5", "claude-sonnet-5-5"), row("claude-opus-4-8", "Opus 4.8", "claude-opus-4-8"),
	]))

private func row(_ value: String, _ label: String, _ id: String) -> ModelRow {
	ModelRow(value: value, label: label, modelId: id)
}

struct ModelSuggestionsTests {
	@Test func anAgentIsOfferedTheTiersThenEveryModelByVersionInTheCoresOrder() {
		let rows = subscription.rows(for: .agentModel)
		#expect(rows.map(\.completion) == [
			"fable", "opus", "sonnet", "haiku", "claude-fable-5-2", "claude-opus-5-5", "claude-sonnet-5-5", "claude-opus-4-8",
		])
		#expect(rows.map(\.section) == Array(repeating: .aliases, count: 4) + Array(repeating: .modelIDs, count: 4))
		#expect((rows[3].name, rows[3].modelID) == ("Haiku (currently Sonnet 5.5)", "claude-sonnet-5-5"))
	}

	@Test func aTierIsOfferedOnlyTheModelsByVersion() {
		let rows = subscription.rows(for: .tier)
		#expect(rows.map(\.completion) == ["claude-fable-5-2", "claude-opus-5-5", "claude-sonnet-5-5", "claude-opus-4-8"])
		#expect(rows.allSatisfy { $0.section == .modelIDs }, "A tier points at a model, not at another tier")
	}

	@Test func typingNarrowsATierRowByItsOwnNameAndAModelRowByItsNameOrID() {
		#expect(
			subscription.rows(for: .agentModel, typed: "son").map(\.completion) == ["sonnet", "claude-sonnet-5-5"],
			"Not \"Haiku (currently Sonnet 5.5)\": a tier is found by its own name")
		#expect(subscription.rows(for: .agentModel, typed: "4.8").map(\.completion) == ["claude-opus-4-8"], "By name too")
		#expect(subscription.rows(for: .agentModel, typed: "nothing-like-it").isEmpty)
		#expect(subscription.rows(for: .agentModel, typed: "opus").count == 8, "A picked value shows every row")
	}

	@Test func theNoteNeverCallsAValueWrong() {
		#expect(subscription.note(for: "sonnet") == "Suggestions from Claude Code 2.1.329. Any model name works.")
		#expect(subscription.note(for: "claude-opus-5-5") == "Suggestions from Claude Code 2.1.329. Any model name works.")
		#expect(subscription.note(for: "my-proxy-model") == "Any model name works.")
		#expect(ModelSuggestions.note(for: "sonnet", listedBy: nil) == "Any model name works.")
		#expect(subscription.note(for: " ") == "Required: pick a model or type its name.")
	}

	@Test func aModelIsNamedByTheCoresRuleADatedSnapshotIncluded() {
		let list = ModelSuggestions(AgentModelList(
			cliVersion: "2.1.326", aliases: [], modelIds: [row("claude-sonnet-4-5", "Sonnet 4.5", "claude-sonnet-4-5")]))
		#expect(list.name(of: "claude-sonnet-4-5-20250929") == "Sonnet 4.5")
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
