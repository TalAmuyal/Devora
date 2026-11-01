import CoreBindings
import Store
import SwiftUI

/// A setting of a config card, Ember's `ConfigFieldSpec`: its key in the core's `CONFIG_FIELDS`, and the control that edits it.
struct ConfigField {
	enum Kind {
		/// [Set | Default]: Set reveals a text input, whose value's surrounding whitespace is trimmed, or kept for a `fragment`, which is concatenated verbatim onto other text.
		case text(placeholder: String, whitespace: Whitespace)
		/// [On | Off | Default].
		case bool
		/// One segment per option.
		case choice([Option])
	}

	/// An option of a `choice`: a stored value, or nil for "Default", which removes the key.
	struct Option: Hashable {
		var label: String
		var value: String?
	}

	var key: String
	var label: String
	/// A muted line under the label.
	var hint: String?
	var kind: Kind
	/// Whether "Default" shows the inherited value; off for a key with a tier the Settings Hub can't see (`pr.auto-merge`'s per-repo override).
	var showsInherited = true
	/// A caveat always shown under the control.
	var note: String?
	/// Shown only while the task tracker resolves to Asana, which a profile may inherit.
	var needsAsana = false

	static func text(_ key: String, _ label: String, hint: String? = nil, placeholder: String = "") -> ConfigField {
		ConfigField(key: key, label: label, hint: hint, kind: .text(placeholder: placeholder, whitespace: .trim))
	}
}

/// How a text input's surrounding whitespace is stored, Ember's `WhitespaceHandling`.
enum Whitespace {
	case trim
	/// For a fragment concatenated verbatim onto other text, such as a PR title prefix.
	case keep

	/// The text as it would be stored, or `""` when it is blank, whitespace-only included.
	func normalize(_ raw: String) -> String {
		if raw.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return "" }
		return self == .trim ? raw.trimmingCharacters(in: .whitespacesAndNewlines) : raw
	}
}

/// A card of settings in the Settings Hub, at either scope.
struct ConfigCard {
	var title: String
	var fields: [ConfigField]

	static let terminal = ConfigCard(title: "Terminal & Session", fields: [
		.text("terminal.default-app", "Default app", hint: "launched per session", placeholder: "e.g. nvim (blank = bare shell)"),
		ConfigField(key: "terminal.git-shortcuts", label: "Git shortcuts", hint: "gst, gd, gri…", kind: .bool),
		.text("prepare-command", "Prepare command", hint: "runs after worktree add", placeholder: "e.g. mise install"),
	])

	static let pullRequests = ConfigCard(title: "Pull Requests", fields: [
		ConfigField(
			key: "pr.auto-merge", label: "Auto-merge", hint: "debi pr submit", kind: .bool, showsInherited: false,
			note: "Per-repo overrides via `debi pr auto-merge` still apply."),
		ConfigField(
			key: "pr.title-prefix", label: "Title prefix", hint: "prepended as-is",
			kind: .text(placeholder: "e.g. \"[ENG] \" (include any trailing space)", whitespace: .keep)),
		.text("feature.branch-prefix", "Branch prefix", hint: "feature-branch name", placeholder: "feature"),
	])

	static let taskTracker = ConfigCard(title: "Task Tracker", fields: [
		ConfigField(key: "task-tracker.provider", label: "Provider", kind: .choice([
			.init(label: "Inherit", value: nil), .init(label: "None", value: ""), .init(label: "Asana", value: "asana"),
		])),
		asana(.text("task-tracker.asana.workspace-id", "Workspace ID")),
		asana(.text("task-tracker.asana.project-id", "Project ID")),
		asana(.text("task-tracker.asana.cli-tag", "CLI tag")),
		asana(.text("task-tracker.asana.section-id", "Section ID", hint: "optional")),
	])

	static let all = [terminal, pullRequests, taskTracker]

	private static func asana(_ field: ConfigField) -> ConfigField {
		var field = field
		field.needsAsana = true
		return field
	}
}

/// The segments of an On/Off/Default setting.
enum BoolChoice: String, CaseIterable {
	case on
	case off
	case `default`

	var label: String { rawValue.capitalized }
}

/// The segments of a text setting.
enum TextMode: String, CaseIterable {
	case set
	case `default`

	var label: String { rawValue.capitalized }
}

/// One scope's config settings in the Settings Hub, Ember's `settingsEditor`: the User Defaults (`profilePath` nil) or a profile's, which its three cards share.
///
/// It writes one key at a time, in order, and reads the scope again after each write, so that every inherited hint is the live resolution.
/// A text setting switched to Set but not committed stays revealed, and what is typed into it stays, across the reloads other settings' writes make.
@MainActor
@Observable
final class ConfigSettingsModel {
	let profilePath: String?
	/// Nil until the first read.
	private(set) var settings: ConfigSettings?
	/// The text settings switched to Set and not committed yet.
	private(set) var pendingEntries: Set<String> = []
	/// What each revealed input holds, by key, kept when the input collapses, so re-entering Set brings it back.
	private(set) var drafts: [String: String] = [:]
	/// The text setting whose input should have the keyboard focus; the cards keep it in step with the field's focus.
	var focusedKey: String?

	@ObservationIgnored private let read: () async -> ConfigSettings?
	@ObservationIgnored private let write: (_ key: String, _ value: String?) async -> Bool
	@ObservationIgnored private var writes: Task<Void, Never>?

	/// `read` and `write` are the core's calls, nil and false when they failed, with their banners.
	init(
		profilePath: String?, read: @escaping () async -> ConfigSettings?,
		write: @escaping (_ key: String, _ value: String?) async -> Bool
	) {
		self.profilePath = profilePath
		self.read = read
		self.write = write
	}

	/// Reads the scope's settings; a failed read keeps the ones it has.
	func reload() async {
		guard let settings = await read() else { return }
		self.settings = settings
	}

	/// Waits for the writes queued so far, and their reloads.
	func settled() async {
		await writes?.value
	}

	func stored(_ key: String) -> ConfigValue? { settings?.stored[key] }

	func resolved(_ key: String) -> ConfigValue? { settings?.resolved[key] ?? nil }

	var usesAsana: Bool { resolved("task-tracker.provider") == .text(value: "asana") }

	func isVisible(_ field: ConfigField) -> Bool { !field.needsAsana || usesAsana }

	// MARK: Text settings

	func storedText(_ key: String) -> String? {
		if case .text(let value) = stored(key) { value } else { nil }
	}

	func textMode(_ key: String) -> TextMode {
		pendingEntries.contains(key) || storedText(key) != nil ? .set : .default
	}

	/// What the revealed input shows: its draft, otherwise the stored value.
	func inputText(_ key: String) -> String { drafts[key] ?? storedText(key) ?? "" }

	func edit(_ key: String, to text: String) { drafts[key] = text }

	/// Set reveals the input, seeded from the stored value, and focuses it; it writes nothing until a value is committed.
	/// Default collapses it, keeping its draft (or the value it removes) for a later Set, and removes the key.
	func selectTextMode(_ mode: TextMode, for key: String) {
		if drafts[key] == nil, let stored = storedText(key) { drafts[key] = stored }
		switch mode {
		case .set:
			pendingEntries.insert(key)
			focusedKey = key
		case .default:
			pendingEntries.remove(key)
			if focusedKey == key { focusedKey = nil }
			persist(key, nil)
		}
	}

	/// Whether the input's Save button shows: only for a non-blank value that differs from what this scope stores, so it never stands for reverting to Default.
	func showsSave(_ key: String, whitespace: Whitespace) -> Bool {
		let value = whitespace.normalize(inputText(key))
		return !value.isEmpty && value != (storedText(key) ?? "")
	}

	/// Enter or Save: a blank value reverts to Default, dropping the draft; any other is stored as normalized, and the input keeps the focus.
	func commit(_ key: String, whitespace: Whitespace) {
		let value = whitespace.normalize(inputText(key))
		if value.isEmpty {
			pendingEntries.remove(key)
			drafts[key] = nil
			persist(key, nil)
		} else {
			drafts[key] = value
			persist(key, value)
		}
	}

	// MARK: Boolean and choice settings

	func boolChoice(_ key: String) -> BoolChoice {
		switch stored(key) {
		case .bool(true): .on
		case .bool(false): .off
		default: .default
		}
	}

	func selectBool(_ choice: BoolChoice, for key: String) {
		switch choice {
		case .on: persist(key, "true")
		case .off: persist(key, "false")
		case .default: persist(key, nil)
		}
	}

	/// The option matching what this scope stores; the first one when none does.
	func chosenOption(_ options: [ConfigField.Option], for key: String) -> ConfigField.Option? {
		let stored = storedText(key)
		return options.first { $0.value == stored } ?? options.first
	}

	func selectOption(_ option: ConfigField.Option, for key: String) {
		persist(key, option.value)
	}

	// MARK: Inheritance

	/// What "Default" inherits, shown beside it: the resolved value, or Devora's default when nothing applies; nil for a field that doesn't show it.
	func inheritedHint(_ field: ConfigField) -> String? {
		guard field.showsInherited else { return nil }
		return Self.describe(resolved(field.key))
	}

	nonisolated static func describe(_ value: ConfigValue?) -> String {
		switch value {
		case nil: "Uses Devora default"
		case .bool(let on): "→ \(on ? "on" : "off")"
		case .text(let text) where text.isEmpty: "→ none"
		case .text(let text): "→ \(text)"
		}
	}

	/// Queues the write behind the earlier ones, then reads the scope again; a failed write, which has its banner, doesn't hold up the next.
	private func persist(_ key: String, _ value: String?) {
		let previous = writes
		writes = Task { [weak self, write] in
			await previous?.value
			guard await write(key, value) else { return }
			await self?.reload()
		}
	}
}

extension ConfigSettingsModel {
	/// A scope over the store's calls.
	convenience init(store: Store, profilePath: String?) {
		self.init(
			profilePath: profilePath,
			read: { await store.configSettings(profilePath: profilePath) },
			write: { key, value in await store.setConfigSetting(profilePath: profilePath, key: key, value: value) })
	}
}
