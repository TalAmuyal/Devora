@testable import AppShell
import CoreBindings
import Layers
import Store
import Testing
import Foundation

/// One scope's config, in memory: writes land in `stored`, and a read resolves each key over `inherited`, as the core's profile → User Defaults → built-in resolution does.
@MainActor
final class FakeScope {
	var stored: [String: ConfigValue] = [:]
	var inherited: [String: ConfigValue] = [:]
	var writes: [(key: String, value: String?)] = []
	var reads = 0
	/// Holds each write until the test lets it through, to check the order they land in.
	var writeGate: (() async -> Void)?

	func model() -> ConfigSettingsModel {
		ConfigSettingsModel(
			profilePath: "/work",
			read: { [self] in
				reads += 1
				let keys = Set(stored.keys).union(inherited.keys)
				return ConfigSettings(stored: stored, resolved: Dictionary(uniqueKeysWithValues: keys.map { ($0, stored[$0] ?? inherited[$0]) }))
			},
			write: { [self] key, value in
				await writeGate?()
				writes.append((key, value))
				switch value {
				case nil: stored[key] = nil
				case "true": stored[key] = .bool(value: true)
				case "false": stored[key] = .bool(value: false)
				case let text?: stored[key] = .text(value: text)
				}
				return true
			})
	}
}

/// The Settings Hub's commit rules for a text input, as Ember's `CommitInput` and `settingsEditor`.
@MainActor
struct ConfigCommitTests {
	let scope = FakeScope()

	func loaded() async -> ConfigSettingsModel {
		let model = scope.model()
		await model.reload()
		return model
	}

	@Test func saveShowsOnlyForANonBlankValueThatDiffersFromTheStoredOne() async {
		scope.stored["prepare-command"] = .text(value: "mise install")
		let model = await loaded()
		model.selectTextMode(.set, for: "prepare-command")
		#expect(model.inputText("prepare-command") == "mise install", "Set seeds the input from the stored value")
		#expect(!model.showsSave("prepare-command", whitespace: .trim))

		model.edit("prepare-command", to: "  mise install  ")
		#expect(!model.showsSave("prepare-command", whitespace: .trim), "trimmed, it is the stored value")
		#expect(model.showsSave("prepare-command", whitespace: .keep), "a fragment's whitespace is part of its value")
		model.edit("prepare-command", to: "   ")
		#expect(!model.showsSave("prepare-command", whitespace: .trim), "blank never shows Save, which would revert to Default")
		model.edit("prepare-command", to: "make")
		#expect(model.showsSave("prepare-command", whitespace: .trim))
	}

	@Test func committingStoresTheNormalizedValueAndKeepsTheInputFocused() async {
		let model = await loaded()
		model.selectTextMode(.set, for: "pr.title-prefix")
		model.edit("pr.title-prefix", to: "[ENG] ")
		model.commit("pr.title-prefix", whitespace: .keep)
		model.selectTextMode(.set, for: "feature.branch-prefix")
		model.edit("feature.branch-prefix", to: "  feat  ")
		model.commit("feature.branch-prefix", whitespace: .trim)
		await model.settled()

		#expect(scope.writes.map(\.value) == ["[ENG] ", "feat"])
		#expect(model.storedText("pr.title-prefix") == "[ENG] ")
		#expect(model.textMode("feature.branch-prefix") == .set)
		#expect(model.focusedKey == "feature.branch-prefix")
	}

	@Test func committingABlankValueRevertsToDefaultAndDropsTheDraft() async {
		scope.stored["prepare-command"] = .text(value: "mise install")
		let model = await loaded()
		model.edit("prepare-command", to: " ")
		model.commit("prepare-command", whitespace: .trim)
		await model.settled()

		#expect(scope.writes.count == 1 && scope.writes[0].value == nil)
		#expect(model.textMode("prepare-command") == .default)
		#expect(model.drafts["prepare-command"] == nil)
	}

	@Test func writesLandOneAtATimeInOrderAndEachIsFollowedByAReread() async {
		var release: [CheckedContinuation<Void, Never>] = []
		scope.writeGate = { await withCheckedContinuation { release.append($0) } }
		let model = await loaded()
		model.selectBool(.off, for: "terminal.git-shortcuts")
		model.selectBool(.on, for: "terminal.git-shortcuts")
		model.selectBool(.default, for: "terminal.git-shortcuts")
		for _ in 0..<3 {
			try? await eventually { release.count == 1 }
			release.removeFirst().resume()
		}
		await model.settled()

		#expect(scope.writes.map(\.value) == ["false", "true", nil])
		#expect(scope.reads == 4, "the first read, then one after each write")
		#expect(model.boolChoice("terminal.git-shortcuts") == .default)
	}

	@Test func aSwitchToSetAndWhatIsTypedSurviveAnotherSettingsWrite() async {
		let model = await loaded()
		model.selectTextMode(.set, for: "terminal.default-app")
		model.edit("terminal.default-app", to: "nv")
		model.selectBool(.off, for: "terminal.git-shortcuts")
		await model.settled()

		#expect(model.textMode("terminal.default-app") == .set)
		#expect(model.inputText("terminal.default-app") == "nv")
		#expect(!scope.writes.contains { $0.key == "terminal.default-app" }, "Set writes nothing until a value is committed")
	}

	@Test func defaultRemovesTheKeyAndReenteringSetBringsBackTheLastValue() async {
		scope.stored["terminal.default-app"] = .text(value: "nvim")
		let model = await loaded()
		model.selectTextMode(.default, for: "terminal.default-app")
		await model.settled()
		#expect(scope.stored["terminal.default-app"] == nil)

		model.selectTextMode(.set, for: "terminal.default-app")
		#expect(model.inputText("terminal.default-app") == "nvim")
	}
}

/// What the cards show for a scope: each control's segment, the inherited hint beside Default, and the Asana settings.
@MainActor
struct ConfigDisplayTests {
	let scope = FakeScope()

	@Test func theSegmentsFollowWhatThisScopeStores() async {
		scope.stored = ["terminal.default-app": .text(value: "hx"), "pr.auto-merge": .bool(value: false), "task-tracker.provider": .text(value: "")]
		scope.inherited = ["prepare-command": .text(value: "mise install"), "terminal.git-shortcuts": .bool(value: true)]
		let model = scope.model()
		await model.reload()
		let provider = ConfigCard.taskTracker.fields[0]
		guard case .choice(let options) = provider.kind else { Issue.record("the provider is a choice"); return }

		#expect(model.textMode("terminal.default-app") == .set)
		#expect(model.textMode("prepare-command") == .default, "an inherited value is still Default here")
		#expect(model.boolChoice("pr.auto-merge") == .off)
		#expect(model.boolChoice("terminal.git-shortcuts") == .default)
		#expect(model.chosenOption(options, for: provider.key)?.label == "None")
		#expect(model.chosenOption(options, for: "task-tracker.unset")?.label == "Inherit")
	}

	@Test func defaultShowsTheInheritedValue() async {
		scope.inherited = [
			"prepare-command": .text(value: "mise install"), "terminal.git-shortcuts": .bool(value: true),
			"pr.auto-merge": .bool(value: true), "task-tracker.provider": .text(value: ""),
		]
		let model = scope.model()
		await model.reload()
		let fields = Dictionary(uniqueKeysWithValues: ConfigCard.all.flatMap(\.fields).map { ($0.key, $0) })

		#expect(model.inheritedHint(fields["prepare-command"]!) == "→ mise install")
		#expect(model.inheritedHint(fields["terminal.git-shortcuts"]!) == "→ on")
		#expect(model.inheritedHint(fields["task-tracker.provider"]!) == "→ none")
		#expect(model.inheritedHint(fields["terminal.default-app"]!) == "Uses Devora default")
		#expect(model.inheritedHint(fields["pr.auto-merge"]!) == nil, "a per-repo override may apply, so auto-merge doesn't say")
	}

	@Test func theAsanaSettingsShowWhileTheProviderResolvesToAsanaEvenInherited() async {
		let model = scope.model()
		let projectID = ConfigCard.taskTracker.fields.first { $0.key == "task-tracker.asana.project-id" }!
		await model.reload()
		#expect(!model.isVisible(projectID))

		scope.inherited["task-tracker.provider"] = .text(value: "asana")
		await model.reload()
		#expect(model.isVisible(projectID))

		model.selectOption(.init(label: "None", value: ""), for: "task-tracker.provider")
		await model.settled()
		#expect(!model.isVisible(projectID))
	}
}

/// The Settings Hub's master list keys, which a field's typing must never reach.
@MainActor
struct SettingsHubKeyTests {
	let j = KeyPress(keyCode: 0x26, characters: "j")
	let d = KeyPress(keyCode: 0x02, characters: "d")
	let escape = KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")

	final class Focus {
		var editing = false
		var cleared = 0
	}

	func hub(_ focus: Focus) async throws -> SettingsHubModel {
		let log = try AppLog.open(path: FileManager.default.temporaryDirectory.appending(path: "settings-\(UUID()).log").path)
		let store = Store(configPath: "/nonexistent/config.json", state: .inMemory, log: log)
		let model = SettingsHubModel(store: store, opening: .list, isEditingText: { focus.editing }, clearFocus: { focus.cleared += 1 })
		model.load()
		try await eventually { model.loaded }
		return model
	}

	@Test func jMovesDownTheListButTypesIntoAFocusedField() async throws {
		let focus = Focus()
		let model = try await hub(focus)
		#expect(model.focusedRow == .userDefaults)

		focus.editing = true
		#expect(!model.handleKey(j), "a key typed into a field goes on to the field")
		#expect(!model.handleKey(d))
		#expect(model.focusedRow == .userDefaults)

		#expect(model.handleKey(escape), "Escape leaves the field")
		#expect(focus.cleared > 0)
		focus.editing = false
		#expect(model.handleKey(j))
		#expect(model.focusedRow == .newProfile)
	}
}
