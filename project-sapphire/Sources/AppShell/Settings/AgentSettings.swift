import CoreBindings
import Foundation
import Observation
import Store

struct AgentSettingsField: Identifiable {
	let key: String
	let label: String
	var id: String { key }
	var multiline: Bool { !["compact-threshold", "template-file"].contains(key) }

	static let all: [Self] = [
		.init(key: "compact-threshold", label: "Compact threshold (%)"),
		.init(key: "template-file", label: "Template file"),
		.init(key: "role", label: "Role"),
		.init(key: "user-details", label: "User details"),
		.init(key: "custom-sections", label: "Custom sections"),
	]
}

struct AgentInputDraft: Equatable {
	var key: String
	var value: String
}

/// One Agent card, sharing the scope's serialized write/reload queue.
@MainActor
@Observable
final class AgentSettingsModel {
	private(set) var settings: AgentSettings?
	private(set) var preview: AgentPreview?
	private(set) var drafts: [String: String] = [:]
	private(set) var pending: Set<String> = []
	private(set) var inputDrafts: [String: AgentInputDraft] = [:]
	private(set) var savingInputs: Set<String> = []
	var newInput = AgentInputDraft(key: "", value: "")
	var focusedKey: String?
	@ObservationIgnored let writes: SettingsWriteQueue
	@ObservationIgnored private let read: () async -> AgentSettings?
	@ObservationIgnored private let write: (String, String?) async -> Bool
	@ObservationIgnored private let writeInput: (String?, String, String?) async -> Bool
	@ObservationIgnored private let reloadScope: () async -> Void
	@ObservationIgnored private let readPreview: (() async -> AgentPreview?)?
	@ObservationIgnored private var reloadVersion: UInt64 = 0
	var canPreview: Bool { readPreview != nil }

	init(
		writes: SettingsWriteQueue, read: @escaping () async -> AgentSettings?,
		write: @escaping (String, String?) async -> Bool,
		writeInput: @escaping (String?, String, String?) async -> Bool,
		reloadScope: @escaping () async -> Void,
		readPreview: (() async -> AgentPreview?)? = nil
	) {
		self.writes = writes
		self.read = read
		self.write = write
		self.writeInput = writeInput
		self.reloadScope = reloadScope
		self.readPreview = readPreview
	}

	func reload() async {
		reloadVersion &+= 1
		let version = reloadVersion
		let value = await read()
		guard version == reloadVersion else { return }
		guard let value else {
			if canPreview { preview = .failure("Could not refresh the saved settings") }
			return
		}
		settings = value
		guard let readPreview else { return }
		preview = nil
		let rendered = await readPreview()
		guard version == reloadVersion else { return }
		preview = rendered ?? .failure("Prompt preview is unavailable")
	}

	func entry(_ key: String) -> AgentSettingEntry? { settings?.fields.first { $0.key == key } }

	func mode(_ key: String) -> TextMode {
		pending.contains(key) || entry(key)?.stored != nil ? .set : .default
	}

	func input(_ key: String) -> String {
		drafts[key] ?? Self.text(entry(key)?.stored ?? entry(key)?.resolved)
	}

	func edit(_ key: String, _ value: String) { drafts[key] = value }

	func select(_ mode: TextMode, key: String) {
		if drafts[key] == nil { drafts[key] = input(key) }
		if mode == .set {
			pending.insert(key)
			focusedKey = key
		} else {
			pending.remove(key)
			persist(key, nil)
		}
	}

	func showsSave(_ field: AgentSettingsField) -> Bool {
		let text = input(field.key)
		if !field.multiline && text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return false }
		if case .cleared? = entry(field.key)?.stored { return true }
		return entry(field.key)?.stored == nil || text != Self.text(entry(field.key)?.stored)
	}

	func commit(_ key: String) {
		let value = key == "compact-threshold" ? input(key).trimmingCharacters(in: .whitespacesAndNewlines) : input(key)
		persist(key, value)
	}

	func resetInputs() { persist("inputs", nil) }

	private func persist(_ key: String, _ value: String?) {
		writes.enqueue { [weak self, write, reloadScope] in
			guard await write(key, value) else { return }
			if value == nil { self?.pending.remove(key) }
			await reloadScope()
		}
	}

	func inputDraft(_ entry: AgentSettingEntry) -> AgentInputDraft {
		inputDrafts[entry.key] ?? .init(key: entry.key, value: Self.text(entry.stored ?? entry.resolved))
	}

	func editInput(_ entry: AgentSettingEntry, key: String? = nil, value: String? = nil) {
		var draft = inputDraft(entry)
		if let key { draft.key = key }
		if let value { draft.value = value }
		inputDrafts[entry.key] = draft
	}

	func saveInput(_ entry: AgentSettingEntry) {
		let draft = inputDraft(entry)
		persistInput(id: entry.key, previous: entry.key, key: draft.key, value: draft.value)
	}

	func defaultInput(_ entry: AgentSettingEntry) {
		persistInput(id: entry.key, previous: entry.key, key: entry.key, value: nil)
	}

	func addInput() {
		persistInput(id: "$new", previous: nil, key: newInput.key, value: newInput.value)
	}

	private func persistInput(id: String, previous: String?, key: String, value: String?) {
		guard savingInputs.insert(id).inserted else { return }
		writes.enqueue { [weak self, writeInput, reloadScope] in
			defer { self?.savingInputs.remove(id) }
			guard await writeInput(previous, key, value) else { return }
			self?.inputDrafts[previous ?? ""] = nil
			if id == "$new" { self?.newInput = .init(key: "", value: "") }
			await reloadScope()
		}
	}

	func hint(_ entry: AgentSettingEntry) -> String {
		let value = entry.resolved.map { Self.text($0) } ?? "No configured value"
		let scope: String = switch entry.source {
		case .default: "Devora default"
		case .global: "User Defaults"
		case .profile: "Profile"
		}
		return "\(value.isEmpty ? "Empty" : value) · \(scope)"
	}

	static func text(_ value: AgentSettingValue?) -> String {
		switch value {
		case .text(let value): value
		case .percent(let value): String(value)
		case .bool(let value): value ? "On" : "Off"
		case .cleared, nil: ""
		}
	}
}
