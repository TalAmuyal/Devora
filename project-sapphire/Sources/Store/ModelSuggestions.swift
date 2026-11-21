import CoreBindings
import Foundation

/// The models Claude Code offers, as suggestions for a model field: grouped, labelled and narrowed to what's typed, for the
/// views to draw. One for each place a model is chosen: an agent's model (New Agent, later its Main Model widget), which
/// takes an alias or a full ID, and a model tier (`ANTHROPIC_DEFAULT_<TIER>_MODEL`), which is what an alias resolves to,
/// so it takes full IDs only.
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

	public struct Section: Equatable, Sendable, Identifiable {
		public let title: String
		public let rows: [Row]
		public var id: String { title }
	}

	public enum Use: Equatable, Sendable {
		case agentModel
		case tier(family: String)
	}

	public let cliVersion: String?
	let models: [AgentModelOption]

	public init(_ list: AgentModelList) {
		cliVersion = list.cliVersion
		models = list.models
	}

	/// The sections for `use`, narrowed to the rows whose ID, alias or name contain `typed` (unless it's a row's own value);
	/// a section left empty is dropped.
	public func sections(for use: Use, typed: String = "") -> [Section] {
		var typed = typed.trimmingCharacters(in: .whitespaces)
		let sections: [Section]
		switch use {
		case .agentModel:
			sections = [
				Section(title: "Claude Code’s models", rows: aliases.map { alias in
					Row(completion: alias.value, name: Self.name(of: alias), alias: alias.value, modelID: alias.resolvedModel ?? alias.value)
				}),
				Section(title: "Other models", rows: others.map { Row(completion: $0.value, name: $0.displayName, alias: nil, modelID: $0.value) }),
			]
		case .tier(let family):
			let ids = fullIDs
			sections = [
				Section(title: "\(family.capitalized) models", rows: ids.filter { $0.modelID.contains(family) }),
				Section(title: "Other models", rows: ids.filter { !$0.modelID.contains(family) }),
			]
		}
		// A value already picked shows every alternative, not only itself.
		if sections.contains(where: { $0.rows.contains { $0.completion == typed } }) { typed = "" }
		return sections.map { section in
			Section(title: section.title, rows: section.rows.filter { row in
				typed.isEmpty || [row.modelID, row.name, row.alias ?? ""].contains { $0.localizedCaseInsensitiveContains(typed) }
			})
		}
		.filter { !$0.rows.isEmpty }
	}

	/// What the note under an agent's model field says about `value`: never that a value is wrong, since any model name
	/// works, and what `opusplan`, the profile's default, which the list doesn't name, does.
	public func note(for value: String) -> String {
		Self.note(for: value, listedBy: self)
	}

	/// The note with no suggestions (none yet, or Claude Code couldn't be asked).
	public static func note(for value: String, listedBy suggestions: ModelSuggestions?) -> String {
		let value = value.trimmingCharacters(in: .whitespaces)
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

	private var aliases: [AgentModelOption] { models.filter(isAlias) }
	private var others: [AgentModelOption] { models.filter { !isAlias($0) } }

	/// Every full ID once, an alias's resolution included, named by its model: `default` names a choice, not a model, so
	/// the model's own alias names it.
	private var fullIDs: [Row] {
		var seen = Set<String>()
		return models.compactMap { model in
			guard model.value != "default" else { return nil }
			let id = model.resolvedModel ?? model.value
			guard seen.insert(id).inserted else { return nil }
			return Row(completion: id, name: model.displayName, alias: nil, modelID: id)
		}
	}

	/// Claude Code's `default` alias is its recommendation; "Default" alone would read as Devora's "follow the profile".
	private static func name(of alias: AgentModelOption) -> String {
		alias.value == "default" ? "Claude Code’s recommended" : alias.displayName
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
