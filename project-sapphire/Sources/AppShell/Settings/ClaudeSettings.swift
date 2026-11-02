import CoreBindings
import Foundation
import Observation
import Store

enum ClaudeMode: String, CaseIterable {
	case custom
	case `default`
	case none
	var label: String { rawValue.capitalized }
}

/// One scope's Claude card, using the same draft/commit contract as the generic cards.
@MainActor
@Observable
final class ClaudeSettingsModel {
	private(set) var settings: ClaudeSettings?
	private(set) var choices: ClaudeSettingChoices?
	private(set) var drafts: [String: String] = [:]
	private(set) var pending: Set<String> = []
	var focusedKey: String?

	@ObservationIgnored private let read: () async -> ClaudeSettings?
	@ObservationIgnored private let readChoices: () async -> ClaudeSettingChoices?
	@ObservationIgnored private let write: (String, ClaudeValue?) async -> Bool
	@ObservationIgnored private let writes: SettingsWriteQueue

	init(
		read: @escaping () async -> ClaudeSettings?, choices: @escaping () async -> ClaudeSettingChoices?,
		write: @escaping (String, ClaudeValue?) async -> Bool, writes: SettingsWriteQueue
	) {
		self.read = read
		self.readChoices = choices
		self.write = write
		self.writes = writes
	}

	func reload() async {
		if choices == nil { choices = await readChoices() }
		if let settings = await read() { self.settings = settings }
	}

	func mode(_ key: String) -> ClaudeMode {
		if pending.contains(key) { return .custom }
		switch settings?.stored[key] {
		case .text?: return .custom
		case .some(.none): return .none
		case nil: return .default
		}
	}

	func storedText(_ key: String) -> String? {
		if case .text(let value)? = settings?.stored[key] { return value }
		return nil
	}

	func input(_ key: String) -> String { drafts[key] ?? storedText(key) ?? "" }
	func edit(_ key: String, _ text: String) { drafts[key] = text }

	func select(_ mode: ClaudeMode, key: String) {
		if drafts[key] == nil, let value = storedText(key) { drafts[key] = value }
		if mode == .custom {
			if drafts[key] == nil, case .text(let inherited)? = settings?.resolved[key] { drafts[key] = inherited }
			pending.insert(key)
			focusedKey = key
		} else {
			pending.remove(key)
			if focusedKey == key { focusedKey = nil }
			persist(key, mode == .none ? .some(.none) : nil)
		}
	}

	func showsSave(_ key: String) -> Bool {
		let value = Whitespace.trim.normalize(input(key))
		return !value.isEmpty && value != storedText(key)
	}

	func commit(_ key: String) {
		let value = Whitespace.trim.normalize(input(key))
		if value.isEmpty {
			pending.remove(key)
			drafts[key] = nil
			persist(key, nil)
		} else {
			drafts[key] = value
			persist(key, .text(value: value))
		}
	}

	var effort: String {
		switch settings?.stored["effort"] {
		case .text(let value)?: value
		case .some(.none): "None"
		case nil: "Default"
		}
	}

	func selectEffort(_ choice: String) {
		persist("effort", choice == "Default" ? nil : choice == "None" ? .some(.none) : .text(value: choice))
	}

	var defaultModelOn: Bool {
		if case .text? = settings?.resolved["default-model"] { return true }
		return false
	}

	func setDefaultModel(_ on: Bool) { persist("default-model", on ? .text(value: "opusplan") : .some(.none)) }

	func inheritedHint(_ key: String) -> String {
		switch settings?.resolved[key] {
		case .text(let value)?: "→ \(value)"
		case .some(.none): "→ none"
		case nil: "Loading…"
		}
	}

	func settled() async { await writes.settled() }

	private func persist(_ key: String, _ value: ClaudeValue?) {
		writes.enqueue { [weak self, write] in
			guard await write(key, value) else { return }
			await self?.reload()
		}
	}
}

extension ClaudeSettingsModel {
	convenience init(store: Store, profilePath: String?, writes: SettingsWriteQueue) {
		self.init(
			read: { await store.claudeSettings(profilePath: profilePath) },
			choices: { await store.claudeSettingChoices() },
			write: { await store.setClaudeSetting(profilePath: profilePath, key: $0, value: $1) }, writes: writes)
	}
}

/// The token's draft is cleared only after a successful save/clear; only presence is read back.
@MainActor
@Observable
final class AsanaTokenModel {
	private(set) var isSet: Bool?
	var draft = ""
	@ObservationIgnored private let status: () async -> Bool?
	@ObservationIgnored private let save: (String) async -> Bool
	@ObservationIgnored private let clear: () async -> Bool
	@ObservationIgnored private let writes: SettingsWriteQueue

	init(status: @escaping () async -> Bool?, save: @escaping (String) async -> Bool, clear: @escaping () async -> Bool, writes: SettingsWriteQueue) {
		self.status = status
		self.save = save
		self.clear = clear
		self.writes = writes
	}

	func reload() async { if let value = await status() { isSet = value } }
	var canSave: Bool { !Whitespace.trim.normalize(draft).isEmpty }

	func saveToken() {
		let token = Whitespace.trim.normalize(draft)
		guard !token.isEmpty else { return }
		writes.enqueue { [weak self, save] in
			guard await save(token) else { return }
			if self?.draft.trimmingCharacters(in: .whitespacesAndNewlines) == token { self?.draft = "" }
			await self?.reload()
		}
	}

	func clearToken() {
		writes.enqueue { [weak self, clear] in
			guard await clear() else { return }
			self?.draft = ""
			await self?.reload()
		}
	}
}

extension AsanaTokenModel {
	convenience init(store: Store, writes: SettingsWriteQueue) {
		self.init(status: { await store.asanaTokenStatus() }, save: { await store.setAsanaToken($0) }, clear: { await store.clearAsanaToken() }, writes: writes)
	}
}
