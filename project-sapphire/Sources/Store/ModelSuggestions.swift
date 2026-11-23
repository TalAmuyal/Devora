import CoreBindings
import Foundation

/// The models Claude Code offers, as every model field shows them (the core's `ModelList::sections`), narrowed to what's
/// typed, for the views to draw. An agent's model (the New Agent dialog, its Model widget) is offered both sections: the
/// tiers Claude Code's aliases name ("Opus (currently 5.5)"; picking one stores the alias, which follows Claude Code to
/// newer models), then every model by its version (picking one pins it). A model tier (`ANTHROPIC_DEFAULT_<TIER>_MODEL`)
/// points at a model, not at another tier, so it's offered the second section only.
public struct ModelSuggestions: Equatable, Sendable {
	/// One suggestion: its section, what picking it puts in the field (`completion`), its label, and the model it runs.
	public struct Row: Equatable, Sendable, Identifiable {
		public enum Section: String, Equatable, Sendable {
			case aliases = "Aliases"
			case modelIDs = "Model IDs"
		}

		public let section: Section
		public let completion: String
		public let name: String
		public let modelID: String
		public var id: String { completion }
	}

	public enum Use: Equatable, Sendable {
		case agentModel
		case tier
	}

	public let cliVersion: String?
	let aliases: [ModelRow]
	let modelIDs: [ModelRow]

	public init(_ list: AgentModelList) {
		cliVersion = list.cliVersion
		aliases = list.aliases
		modelIDs = list.modelIds
	}

	/// The name Claude Code gives the model `id` (a full ID), by the core's rule: a dated snapshot is its undated model.
	public func name(of id: String) -> String? {
		(try? modelName(models: modelIDs, id: id)) ?? nil
	}

	/// The rows for `use`, in the core's order, narrowed to what's `typed` by the core's rule for every model field (a tier by
	/// its own name, a model by its name or ID; every row while the field holds exactly a value one of them stores).
	public func rows(for use: Use, typed: String = "") -> [Row] {
		let offered = AgentModelList(cliVersion: cliVersion, aliases: use == .agentModel ? aliases : [], modelIds: modelIDs)
		let shown = (try? narrowModels(list: offered, typed: typed)) ?? offered
		return shown.aliases.map { Row(section: .aliases, completion: $0.value, name: $0.label, modelID: $0.modelId) }
			+ shown.modelIds.map { Row(section: .modelIDs, completion: $0.value, name: $0.label, modelID: $0.modelId) }
	}

	/// What the note under an agent's model field says about `value`: that one is required when it's empty, never that a
	/// value is wrong, since any model name works.
	public func note(for value: String) -> String {
		Self.note(for: value, listedBy: self)
	}

	/// The note with no suggestions (none yet, or Claude Code couldn't be asked).
	public static func note(for value: String, listedBy suggestions: ModelSuggestions?) -> String {
		let value = value.trimmingCharacters(in: .whitespaces)
		if value.isEmpty { return "Required: pick a model or type its name." }
		guard let suggestions, let version = suggestions.cliVersion, suggestions.lists(value) else { return "Any model name works." }
		return "Suggestions from Claude Code \(version). Any model name works."
	}

	func lists(_ value: String) -> Bool {
		(aliases + modelIDs).contains { $0.value == value || $0.modelId == value }
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
