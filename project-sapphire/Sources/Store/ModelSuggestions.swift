import CoreBindings
import Foundation

/// The models Claude Code offers, as suggestions for a model field: one list, ordered, labelled and narrowed to what's
/// typed, for the views to draw. For each place a model is chosen: an agent's model (New Agent, later its Main Model
/// widget), which takes an alias or a full ID, and a model tier (`ANTHROPIC_DEFAULT_<TIER>_MODEL`), which is what an alias
/// resolves to, so it takes full IDs only.
public struct ModelSuggestions: Equatable, Sendable {
	/// One suggestion: what picking it puts in the field (`completion`), its name and, beside it, the stored alias, and its
	/// full model ID.
	public struct Row: Equatable, Sendable, Identifiable {
		public let completion: String
		public let name: String
		public let alias: String?
		public let modelID: String
		public var id: String { completion }
	}

	public enum Use: Equatable, Sendable {
		case agentModel
		case tier
	}

	public let cliVersion: String?
	let models: [AgentModelOption]

	public init(_ list: AgentModelList) {
		cliVersion = list.cliVersion
		models = list.models
	}

	/// The rows for `use`, newest generation first, then by tier (Fable, Opus, Sonnet, Haiku), then any other model by ID,
	/// narrowed to the rows whose ID, alias or name contain `typed` (unless it's a row's own value).
	public func rows(for use: Use, typed: String = "") -> [Row] {
		var typed = typed.trimmingCharacters(in: .whitespaces)
		let rows: [Row]
		switch use {
		case .agentModel:
			// Claude Code's `default` is a choice, not a model: an agent is created with a model chosen.
			rows = models.filter { $0.value != "default" }.map { model in
				Row(
					completion: model.value, name: Self.name(of: model), alias: isAlias(model) ? model.value : nil,
					modelID: model.resolvedModel ?? model.value)
			}
		case .tier:
			rows = fullIDs
		}
		// A value already picked shows every alternative, not only itself.
		if rows.contains(where: { $0.completion == typed }) { typed = "" }
		return rows
			.filter { row in typed.isEmpty || [row.modelID, row.name, row.alias ?? ""].contains { $0.localizedCaseInsensitiveContains(typed) } }
			.sorted { Self.precedes($0.modelID, $1.modelID) }
	}

	/// The order Claude's models read in: `claude-<tier>-<major>[-<minor>][-<date>]` by generation, newest first, then by
	/// tier; every other ID after them, by ID.
	public static func precedes(_ first: String, _ second: String) -> Bool {
		switch (Generation(first), Generation(second)) {
		case (let a?, let b?): a.sortsBefore(b) || (!b.sortsBefore(a) && first < second)
		case (.some, nil): true
		case (nil, .some): false
		case (nil, nil): first < second
		}
	}

	struct Generation {
		static let tiers = ["fable", "opus", "sonnet", "haiku"]
		let major: Int
		let minor: Int
		let tier: Int

		init?(_ id: String) {
			let parts = id.split(separator: "-").map(String.init)
			guard parts.count >= 3, parts[0] == "claude", let tier = Self.tiers.firstIndex(of: parts[1]), let major = Int(parts[2])
			else { return nil }
			// A date suffix (`-20251001`) isn't a minor version.
			let minor = parts.count > 3 ? Int(parts[3]).flatMap { $0 < 1000 ? $0 : nil } ?? 0 : 0
			(self.major, self.minor, self.tier) = (major, minor, tier)
		}

		func sortsBefore(_ other: Generation) -> Bool {
			(other.major, other.minor, tier) < (major, minor, other.tier)
		}
	}

	/// What the note under an agent's model field says about `value`: that one is required when it's empty, never that a
	/// value is wrong, since any model name works, and what `opusplan`, which the list doesn't name, does.
	public func note(for value: String) -> String {
		Self.note(for: value, listedBy: self)
	}

	/// The note with no suggestions (none yet, or Claude Code couldn't be asked).
	public static func note(for value: String, listedBy suggestions: ModelSuggestions?) -> String {
		let value = value.trimmingCharacters(in: .whitespaces)
		if value.isEmpty { return "Required: pick a model or type its name." }
		if value == "opusplan" { return "Opus while planning, Sonnet for the rest." }
		guard let suggestions, let version = suggestions.cliVersion, suggestions.lists(value) else { return "Any model name works." }
		return "Suggestions from Claude Code \(version). Any model name works."
	}

	func lists(_ value: String) -> Bool {
		models.contains { $0.value == value || $0.resolvedModel == value }
	}

	/// An alias names a model it resolves to (`opus` → `claude-opus-5-5`); every other entry is a full ID.
	private func isAlias(_ model: AgentModelOption) -> Bool {
		model.resolvedModel.map { $0 != model.value } ?? false
	}

	/// Every full ID once, an alias's resolution included, named by its model: `default` names a choice, not a model, so
	/// the model's own alias names it.
	private var fullIDs: [Row] {
		var seen = Set<String>()
		return models.compactMap { model in
			guard model.value != "default" else { return nil }
			let id = model.resolvedModel ?? model.value
			guard seen.insert(id).inserted else { return nil }
			return Row(completion: id, name: Self.name(of: model), alias: nil, modelID: id)
		}
	}

	/// A model's name, or its ID when the list gives it none.
	private static func name(of model: AgentModelOption) -> String {
		model.displayName.isEmpty ? model.value : model.displayName
	}
}

extension Store {
	/// The models Claude Code offers an agent of `profilePath` (the selected profile's by default), or nil when it can't be
	/// asked (no CLI, not logged in, a timeout): suggestions are optional, so that's a warning in the log, not a banner.
	/// The core caches the list, so only a cold call waits for Claude Code.
	///
	/// A test run (`DEVORA_TEST_MODE`) that names no fake CLI (`DEVORA_TEST_CLAUDE_PROGRAM`) looks up nothing: the lookup
	/// would otherwise launch the user's own `claude`, with their login.
	public func modelSuggestions(
		profilePath: String? = nil, environment: [String: String] = ProcessInfo.processInfo.environment
	) async -> ModelSuggestions? {
		guard Self.mayLookUpModels(environment), let core, let profile = profilePath ?? selectedProfilePath else { return nil }
		do {
			let launch = try await core.resolveAgentLaunch(resourcesDir: Bundle.main.resourcePath)
			return ModelSuggestions(try await core.agentModelOptions(profilePath: profile, launch: launch))
		} catch {
			logWarning("No model suggestions for \(profile): \(Self.describe(error))")
			return nil
		}
	}

	static func mayLookUpModels(_ environment: [String: String]) -> Bool {
		environment["DEVORA_TEST_MODE"] != "1" || environment["DEVORA_TEST_CLAUDE_PROGRAM"] != nil
	}
}
