@testable import AppShell
import CoreBindings
import Testing

@MainActor
private final class ClaudeScope {
	var stored: [String: ClaudeValue] = [:]
	var inherited: [String: ClaudeValue] = ["default-model": .text(value: "opusplan"), "effort": .text(value: "xhigh")]
	var writes: [(String, ClaudeValue?)] = []
	var fail = false

	func model(queue: SettingsWriteQueue = SettingsWriteQueue()) -> ClaudeSettingsModel {
		ClaudeSettingsModel(
			read: { [self] in ClaudeSettings(stored: stored, resolved: inherited.merging(stored) { _, value in value }) },
			choices: { ClaudeSettingChoices(effortLevels: ["low", "max"], modelSuggestions: ["suggested-id"], permissionModeSuggestions: ["plan"]) },
			write: { [self] key, value in
				guard !fail else { return false }
				writes.append((key, value))
				stored[key] = value
				return true
			}, writes: queue)
	}
}

@MainActor
struct ClaudeSettingsTests {
	@Test func customStartsWithTheInheritedValueAndAnyResolvedDefaultModelIsOn() async {
		let scope = ClaudeScope()
		scope.inherited["opus-model"] = .text(value: "inherited-id")
		scope.inherited["default-model"] = .text(value: "hand-edited-model")
		let model = scope.model()
		await model.reload()
		model.select(.custom, key: "opus-model")
		#expect(model.input("opus-model") == "inherited-id")
		#expect(scope.writes.isEmpty)
		#expect(model.defaultModelOn)
	}

	@Test func noneAndDefaultAreDistinctAndCustomDraftsSurviveOtherWrites() async {
		let scope = ClaudeScope()
		let model = scope.model()
		await model.reload()
		#expect(model.mode("opus-model") == .default)
		model.select(.custom, key: "opus-model")
		model.edit("opus-model", "future-id")
		model.selectEffort("max")
		await model.settled()
		#expect(scope.stored["opus-model"] == nil, "selecting Custom only reveals the input")
		#expect(model.input("opus-model") == "future-id")
		#expect(model.mode("opus-model") == .custom)
		model.commit("opus-model")
		await model.settled()
		#expect(scope.stored["opus-model"] == .text(value: "future-id"))
		model.select(.none, key: "opus-model")
		await model.settled()
		#expect(scope.stored["opus-model"] == .some(.none))
		#expect(model.mode("opus-model") == .none)
		model.select(.default, key: "opus-model")
		await model.settled()
		#expect(scope.stored["opus-model"] == nil)
		model.select(.custom, key: "opus-model")
		#expect(model.input("opus-model") == "future-id")
	}

	@Test func blankCommitsInheritAndPermissionValuesAreFreeText() async {
		let scope = ClaudeScope()
		let model = scope.model()
		await model.reload()
		model.select(.custom, key: "permission-mode")
		model.edit("permission-mode", "  future-permission  ")
		#expect(model.showsSave("permission-mode"))
		model.commit("permission-mode")
		await model.settled()
		#expect(scope.stored["permission-mode"] == .text(value: "future-permission"))
		#expect(!model.showsSave("permission-mode"))
		model.edit("permission-mode", " \n ")
		model.commit("permission-mode")
		await model.settled()
		#expect(scope.stored["permission-mode"] == nil)
		#expect(model.mode("permission-mode") == .default)
		#expect(model.drafts["permission-mode"] == nil)
		model.setDefaultModel(false)
		await model.settled()
		#expect(!model.defaultModelOn)
		#expect(scope.stored["default-model"] == .some(.none))
		model.setDefaultModel(true)
		await model.settled()
		#expect(scope.stored["default-model"] == .text(value: "opusplan"))
	}

	@Test func failedWritesKeepDraftAndStoredValue() async {
		let scope = ClaudeScope()
		scope.fail = true
		let model = scope.model()
		await model.reload()
		model.select(.custom, key: "haiku-model")
		model.edit("haiku-model", "new-id")
		model.commit("haiku-model")
		await model.settled()
		#expect(scope.writes.isEmpty)
		#expect(model.input("haiku-model") == "new-id")
		#expect(model.showsSave("haiku-model"))
	}

	@Test func cardsShareOneWriteQueueAndTokenFailuresKeepTheDraft() async {
		let queue = SettingsWriteQueue()
		var events: [String] = []
		var release: CheckedContinuation<Void, Never>?
		queue.enqueue {
			events.append("first")
			await withCheckedContinuation { release = $0 }
		}
		var present = false
		var fails = true
		let token = AsanaTokenModel(
			status: { present }, save: { _ in
				events.append("token")
				if fails { return false }
				present = true
				return true
			}, clear: { present = false; return true }, writes: queue)
		token.draft = "test-token"
		token.saveToken()
		for _ in 0..<50 where release == nil { await Task.yield() }
		#expect(events == ["first"], "a later card's write waits for the earlier one")
		release?.resume()
		await queue.settled()
		#expect(events == ["first", "token"])
		#expect(token.draft == "test-token")
		fails = false
		token.saveToken()
		await queue.settled()
		#expect(token.isSet == true)
		#expect(token.draft.isEmpty)
		token.clearToken()
		await queue.settled()
		#expect(token.isSet == false)
	}
}
