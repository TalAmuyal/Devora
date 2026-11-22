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
	let tierModels: [TierModel]

	public init(_ list: AgentModelList) {
		cliVersion = list.cliVersion
		models = list.models
		tierModels = list.tierModels
	}

	/// The rows for `use`, in the core's order (newest generation first, then by tier, then any other model by ID),
	/// narrowed to the rows whose ID, alias or name contain `typed` (unless it's a row's own value).
	public func rows(for use: Use, typed: String = "") -> [Row] {
		var typed = typed.trimmingCharacters(in: .whitespaces)
		let rows: [Row]
		switch use {
		case .agentModel:
			// The core leaves out Claude Code's `default`, a choice rather than a model: an agent is created with a model chosen.
			rows = models.map { model in
				let isAlias = model.resolvedModel.map { $0 != model.value } ?? false
				return Row(
					completion: model.value, name: model.displayName, alias: isAlias ? model.value : nil,
					modelID: model.resolvedModel ?? model.value)
			}
		case .tier:
			rows = tierModels.map { Row(completion: $0.id, name: $0.name, alias: nil, modelID: $0.id) }
		}
		// A value already picked shows every alternative, not only itself.
		if rows.contains(where: { $0.completion == typed }) { typed = "" }
		return rows.filter { row in
			typed.isEmpty || [row.modelID, row.name, row.alias ?? ""].contains { $0.localizedCaseInsensitiveContains(typed) }
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
		models.contains { $0.value == value || $0.resolvedModel == value } || tierModels.contains { $0.id == value }
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
