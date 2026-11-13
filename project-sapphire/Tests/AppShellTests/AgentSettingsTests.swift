@testable import AppShell
import CoreBindings
import Store
import Testing
import TestSupport

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct AgentSettingsTests {
	func settings(stored: AgentSettingValue? = nil) -> AgentSettings {
		AgentSettings(fields: [
			AgentSettingEntry(key: "role", stored: stored, resolved: .text(value: "Inherited role"), source: .global),
			AgentSettingEntry(key: "compact-threshold", stored: nil, resolved: .percent(value: 80), source: .default),
		], inputs: [], inputsCleared: false)
	}

	@Test func setSeedsTheResolvedValueAndMultilineCommitsVerbatimIncludingEmpty() async {
		let queue = SettingsWriteQueue()
		var saved: [(String, String?)] = []
		let model = AgentSettingsModel(
			writes: queue, read: { settings() },
			write: { key, value in saved.append((key, value)); return true },
			writeInput: { _, _, _ in true }, reloadScope: {})
		await model.reload()
		model.select(.set, key: "role")
		#expect(model.input("role") == "Inherited role")
		model.edit("role", "\n role \n")
		model.commit("role")
		await queue.settled()
		#expect(saved.last?.1 == "\n role \n")
		model.edit("role", "")
		#expect(model.showsSave(AgentSettingsField.all.first { $0.key == "role" }!))
		model.commit("role")
		await queue.settled()
		#expect(saved.last?.1 == "")
		model.select(.default, key: "role")
		await queue.settled()
		#expect(saved.last?.1 == nil)
	}

	@Test func explicitNullIsDistinctFromDefaultAndCanBeReplacedWithEmptyText() async {
		let model = AgentSettingsModel(
			writes: SettingsWriteQueue(), read: { settings(stored: .cleared) },
			write: { _, _ in true }, writeInput: { _, _, _ in true }, reloadScope: {})
		await model.reload()
		#expect(model.mode("role") == .set)
		#expect(model.input("role") == "")
		#expect(model.showsSave(AgentSettingsField.all.first { $0.key == "role" }!))
		#expect(model.hint(model.entry("compact-threshold")!) == "80 · Devora default")
	}

	@Test func failedWritesRetainFieldAndMapDraftsAndDoNotReload() async {
		let queue = SettingsWriteQueue()
		var reloads = 0
		let model = AgentSettingsModel(
			writes: queue, read: { settings() }, write: { _, _ in false },
			writeInput: { _, _, _ in false }, reloadScope: { reloads += 1 })
		await model.reload()
		model.select(.set, key: "role")
		model.edit("role", "draft")
		model.commit("role")
		model.newInput = .init(key: "repo.notes", value: "\n notes \n")
		model.addInput()
		await queue.settled()
		#expect(model.input("role") == "draft")
		#expect(model.newInput == .init(key: "repo.notes", value: "\n notes \n"))
		#expect(model.savingInputs.isEmpty)
		#expect(reloads == 0)
	}

	@Test func writesShareTheOtherCardsQueueAndIncludeEachReload() async {
		let queue = SettingsWriteQueue()
		var operations: [String] = []
		let generic = ConfigSettingsModel(
			profilePath: "/profile", read: { operations.append("generic read"); return ConfigSettings(stored: [:], resolved: [:]) },
			write: { _, _ in operations.append("generic write"); return true }, writeQueue: queue)
		let agent = AgentSettingsModel(
			writes: queue, read: { settings() },
			write: { _, _ in operations.append("agent write"); return true },
			writeInput: { _, _, _ in true }, reloadScope: { operations.append("agent read") })
		agent.edit("role", "role")
		agent.commit("role")
		generic.edit("prepare-command", to: "mise install")
		generic.commit("prepare-command", whitespace: .trim)
		await queue.settled()
		#expect(operations == ["agent write", "agent read", "generic write", "generic read"])
	}

	@Test func unrelatedReloadsPreserveUncommittedDraftsAndMapRenamesUseOneWrite() async {
		let queue = SettingsWriteQueue()
		let entry = AgentSettingEntry(key: "repo.notes", stored: .text(value: "old"), resolved: .text(value: "old"), source: .profile)
		var writes: [(String?, String, String?)] = []
		let model = AgentSettingsModel(
			writes: queue, read: { AgentSettings(fields: settings().fields, inputs: [entry], inputsCleared: false) },
			write: { _, _ in true },
			writeInput: { old, key, value in writes.append((old, key, value)); return true }, reloadScope: {})
		await model.reload()
		model.select(.set, key: "role")
		model.edit("role", "unsaved role")
		model.editInput(entry, key: "repo.other", value: "new")
		await model.reload()
		#expect(model.input("role") == "unsaved role")
		#expect(model.inputDraft(entry).key == "repo.other")
		model.saveInput(entry)
		await queue.settled()
		#expect(writes.count == 1)
		#expect(writes[0].0 == "repo.notes")
		#expect(writes[0].1 == "repo.other")
		#expect(writes[0].2 == "new")
	}

	@Test func previewReadsSavedValuesOnlyAndRefreshesAfterAQueuedWrite() async {
		let queue = SettingsWriteQueue()
		var saved = "saved role"
		var renders = 0
		weak var owner: AgentSettingsModel?
		let model = AgentSettingsModel(
			writes: queue, read: { settings(stored: .text(value: saved)) },
			write: { _, value in saved = value ?? "default role"; return true },
			writeInput: { _, _, _ in true }, reloadScope: { await owner?.reload() },
			readPreview: { renders += 1; return .text(saved) })
		owner = model
		await model.reload()
		#expect(model.preview == .text("saved role"))
		model.edit("role", "unsaved role")
		#expect(model.preview == .text("saved role"))
		#expect(renders == 1)
		model.commit("role")
		await queue.settled()
		#expect(model.preview == .text("unsaved role"))
		#expect(renders == 2)
	}

	@Test func aNewPreviewOrFailureCannotBeReplacedByAnOlderRender() async throws {
		var first: CheckedContinuation<AgentPreview?, Never>?
		var renders = 0
		let model = AgentSettingsModel(
			writes: SettingsWriteQueue(), read: { settings() }, write: { _, _ in true },
			writeInput: { _, _, _ in true }, reloadScope: {},
			readPreview: {
				renders += 1
				if renders == 1 { return await withCheckedContinuation { first = $0 } }
				return .failure("missing template value: team")
			})
		let initial = Task { await model.reload() }
		try await eventually { first != nil }
		await model.reload()
		#expect(model.preview == .failure("missing template value: team"))
		first?.resume(returning: .text("old render"))
		await initial.value
		#expect(model.preview == .failure("missing template value: team"))
	}
}
