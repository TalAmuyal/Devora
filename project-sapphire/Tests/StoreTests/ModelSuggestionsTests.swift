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

private func completions(_ sections: [ModelSuggestions.Section]) -> [String: [String]] {
	Dictionary(uniqueKeysWithValues: sections.map { ($0.title, $0.rows.map(\.completion)) })
}

struct ModelSuggestionsTests {
	@Test func anAgentsModelIsOfferedTheAliasesThenTheOtherModelsAndAPickStoresTheAlias() throws {
		let sections = subscription.sections(for: .agentModel)
		#expect(sections.map(\.title) == ["Claude Code’s models", "Other models"])
		#expect(sections[0].rows.map(\.completion) == ["default", "opus", "sonnet", "haiku"])
		#expect(sections[1].rows.map(\.completion) == ["claude-haiku-4-5-20251001", "claude-opus-4-8", "claude-sonnet-4-6"])
		let opus = sections[0].rows[1]
		#expect((opus.name, opus.alias, opus.modelID) == ("Opus 5.5", "opus", "claude-opus-5-5"))
		#expect(
			sections[0].rows[0].name == "Claude Code’s recommended",
			"Claude Code's default alias mustn't read as Devora's Default (follow the profile)")
	}

	@Test func aTierIsOfferedEachFullIDOnceItsOwnFamilyFirst() {
		let sections = subscription.sections(for: .tier(family: "opus"))
		#expect(completions(sections) == [
			"Opus models": ["claude-opus-5-5", "claude-opus-4-8"],
			"Other models": ["claude-sonnet-5-5", "claude-haiku-5-5", "claude-haiku-4-5-20251001", "claude-sonnet-4-6"],
		])
		#expect(sections.map(\.title) == ["Opus models", "Other models"])
		#expect(sections[0].rows[0].name == "Opus 5.5", "An alias's model is named by its alias, not by `default`")
	}

	@Test func aModelWithoutANameIsNamedByItsID() {
		let unnamed = ModelSuggestions(AgentModelList(cliVersion: nil, models: [option("claude-next", "claude-next", "")]))
		#expect(unnamed.sections(for: .agentModel).flatMap(\.rows).map(\.name) == ["claude-next"])
		#expect(unnamed.sections(for: .tier(family: "opus")).flatMap(\.rows).map(\.name) == ["claude-next"])
	}

	@Test func theMenuNarrowsToWhatIsTypedUnlessItIsAlreadyAPick() {
		#expect(completions(subscription.sections(for: .agentModel, typed: "son")) == [
			"Claude Code’s models": ["sonnet"], "Other models": ["claude-sonnet-4-6"],
		])
		#expect(subscription.sections(for: .agentModel, typed: "4.8").flatMap(\.rows).map(\.completion) == ["claude-opus-4-8"], "By name too")
		#expect(subscription.sections(for: .agentModel, typed: "nothing-like-it").isEmpty)
		#expect(subscription.sections(for: .agentModel, typed: "opus").flatMap(\.rows).count == 7, "A picked value shows every alternative")
	}

	@Test func theNoteNeverCallsAValueWrong() {
		#expect(subscription.note(for: "sonnet") == "Suggestions from Claude Code 2.1.312. Any model name works.")
		#expect(subscription.note(for: "claude-opus-5-5") == "Suggestions from Claude Code 2.1.312. Any model name works.")
		#expect(subscription.note(for: "opusplan") == "Opus while planning, Sonnet for the rest.")
		#expect(subscription.note(for: "my-proxy-model") == "Any model name works.")
		#expect(ModelSuggestions.note(for: "opusplan", listedBy: nil) == "Opus while planning, Sonnet for the rest.")
		#expect(ModelSuggestions.note(for: "sonnet", listedBy: nil) == "Any model name works.")
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
