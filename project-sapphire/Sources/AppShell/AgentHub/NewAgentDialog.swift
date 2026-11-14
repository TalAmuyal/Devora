import CoreBindings
import Layers
import Store
import SwiftUI

/// One open New Agent dialog (ADR-012): the name, the initial repos, and the template's fields, as a modal layer over the Agent Hub, then the creation's progress in its place.
///
/// The fields come from the template (`agent_creation_fields`): the ones only the form fills first, then the pre-filled ones, editable for this agent, each with a button that saves its value to its source; the derived ones are read-only.
/// Its keys follow the New Task dialog's: Escape cancels and Enter creates, except that Enter toggles the active repo while the filter has the focus, where ↑ and ↓ move it; Tab and Shift+Tab cycle the fields and buttons.
@MainActor
@Observable
final class NewAgentDialogModel {
	static let layerName = "new-agent-dialog"

	enum Focus: Hashable {
		case name
		case field(String)
		case model
		case filter
		case cancel
		case create

		var isTextField: Bool {
			switch self {
			case .name, .field, .model, .filter: true
			case .cancel, .create: false
			}
		}
	}

	let repos: [RepoInfo]
	var name = "" {
		didSet { if name != oldValue { refreshFields() } }
	}
	var filter = "" {
		didSet { if filter != oldValue { cursor.select(0) } }
	}
	var focus = Focus.name {
		didSet { if !focus.isTextField { clearFocus() } }
	}
	private(set) var selected: Set<String> = []
	private(set) var cursor = ListCursor()
	/// The template's fields, as the core last listed them.
	private(set) var fields: [AgentField] = []
	/// What the user typed into each editable field, by key; a field not in it shows its listed value.
	var edits: [String: String] = [:] {
		didSet { if edits != oldValue { refreshPreview() } }
	}
	/// Whether the instructions keep Claude Code's own coding instructions, appended to them, or replace them.
	var keepCodingInstructions = true
	/// The model every launch of the agent uses: free text, so an alias or a full model name both work. Empty until the core's default arrives.
	var model = ""
	/// The effort every launch uses; `nil` leaves it to Claude Code.
	private(set) var effort: String?
	/// The efforts the slider offers after "Claude Code default", low → max.
	private(set) var effortLevels: [String] = []
	/// The instructions as creation would render them now, or why it would fail.
	private(set) var preview: AgentPreview?
	/// The fields whose value was saved to its source, for their button's label.
	private(set) var saved: Set<String> = []
	/// The creation, once started; the dialog shows its progress instead of the form.
	private(set) var run: AgentCreationRun?

	@ObservationIgnored let store: Store
	@ObservationIgnored var close: () -> Void = {}
	/// Opens the created agent's tab and closes the hub.
	@ObservationIgnored var opened: (String) -> Void = { _ in }
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private var fieldsTask: Task<Void, Never>?
	@ObservationIgnored private var previewTask: Task<Void, Never>?
	/// Set once the user moves the effort slider, so a late default doesn't override it.
	@ObservationIgnored private var effortChosen = false

	init(store: Store, repos: [RepoInfo], clearFocus: @escaping () -> Void) {
		self.store = store
		self.repos = repos
		self.clearFocus = clearFocus
		Task { [weak self, store] in
			guard let defaults = await store.agentModelDefaults() else { return }
			self?.applyModelDefaults(defaults)
		}
	}

	private func applyModelDefaults(_ defaults: AgentModelDefaults) {
		effortLevels = defaults.effortLevels
		if model.isEmpty { model = defaults.choice.model }
		if !effortChosen { effort = defaults.choice.effort }
	}

	/// The user's effort choice, which a late default no longer overrides.
	func chooseEffort(_ effort: String?) {
		effortChosen = true
		self.effort = effort
	}

	var visibleRepos: [RepoInfo] { repos.matching(filter) }

	var activeRepo: RepoInfo? {
		let visible = visibleRepos
		return cursor.position(in: visible.count).map { visible[$0] }
	}

	var editableFields: [AgentField] { fields.filter { $0.source != .derived } }
	/// The derived fields worth showing: the name is the Name field itself, and the space's path shows under it.
	var derivedFields: [AgentField] {
		fields.filter { $0.source == .derived && !["agent.name", "space.path"].contains($0.key) }
	}

	/// Where the agent's space will be created, shown under its name.
	var spacePath: String? {
		guard !trimmedName.isEmpty, let profile = store.selectedProfilePath else { return nil }
		return ((profile as NSString).appendingPathComponent("agent-spaces/\(trimmedName)") as NSString).abbreviatingWithTildeInPath
	}

	func value(of field: AgentField) -> String { edits[field.key] ?? field.value ?? "" }

	/// A name, a model, and a value for every field only the form can fill; a pre-filled field may be empty, as an unset optional section is.
	var canCreate: Bool {
		!trimmedName.isEmpty && !trimmedModel.isEmpty && editableFields.allSatisfy { $0.source != nil || !value(of: $0).trimmingCharacters(in: .whitespaces).isEmpty }
	}

	/// Where "Save" writes a field: back to the global config when it came from there, otherwise to the profile.
	func saveScope(of field: AgentField) -> AgentConfigScope {
		field.source == .global ? .global : .profile
	}

	func toggle(_ repo: RepoInfo) {
		if selected.remove(repo.path) == nil { selected.insert(repo.path) }
		refreshFields()
	}

	/// A click on a repo's checkbox or name toggles it and makes it the active one.
	func click(_ repo: RepoInfo) {
		toggle(repo)
		activate(repo)
	}

	/// Makes `repo` the active row, as selecting it in the list does, without toggling it.
	func activate(_ repo: RepoInfo) {
		if let index = visibleRepos.firstIndex(of: repo) { cursor.select(index) }
	}

	/// Lists the fields again for the current name and repos, which the derived ones show; the edits stay.
	func refreshFields() {
		fieldsTask?.cancel()
		let form = AgentCreationForm(
			name: trimmedName, role: nil, repos: selectedRepoPaths, inputs: [:], keepCodingInstructions: keepCodingInstructions, model: nil)
		fieldsTask = Task { [weak self, store] in
			guard let fields = await store.agentCreationFields(form), !Task.isCancelled else { return }
			self?.fields = fields
			self?.refreshPreview()
		}
	}

	/// Renders the instructions again for the current form; a later request wins.
	func refreshPreview() {
		previewTask?.cancel()
		let form = currentForm
		previewTask = Task { [weak self, store] in
			guard let preview = await store.agentCreationPreview(form), !Task.isCancelled else { return }
			self?.preview = preview
		}
	}

	func save(_ field: AgentField) {
		let value = value(of: field)
		let scope = saveScope(of: field)
		Task { [weak self, store] in
			guard await store.saveAgentField(scope: scope, key: field.key, value: value) else { return }
			self?.saved.insert(field.key)
		}
	}

	func create() {
		guard run == nil, canCreate else { return }
		let form = currentForm
		let run = AgentCreationRun()
		self.run = run
		focus = .cancel
		Task { [weak self, store] in
			await store.createAgentSpace(form, into: run)
			switch run.outcome {
			case .done(let name): self?.opened(name)
			case .cancelled: self?.close()
			case .failed, nil: break
			}
		}
	}

	/// Before the creation: closes the dialog; during it: cancels it; after a failure: closes the dialog.
	func cancel() {
		if let run, run.isRunning {
			store.cancelAgentCreation(run)
		} else {
			close()
		}
	}

	func cycleFocus(backward: Bool) {
		guard run == nil else { return }
		var order: [Focus] = [.name]
		order += editableFields.map { .field($0.key) }
		if !repos.isEmpty { order.append(.filter) }
		order.append(.model)
		order += [.cancel, .create]
		let index = order.firstIndex(of: focus) ?? 0
		focus = order[(index + (backward ? order.count - 1 : 1)) % order.count]
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		if run != nil {
			// The progress has one button, which Escape, Space and Enter press.
			guard [KeyCode.escape, KeyCode.space, KeyCode.returnKey, KeyCode.keypadEnter].contains(key.keyCode) else {
				return key.isUnusedOnAPage
			}
			cancel()
			return true
		}
		switch key.keyCode {
		case KeyCode.escape:
			cancel()
		case KeyCode.returnKey, KeyCode.keypadEnter:
			if focus == .filter {
				if let activeRepo { toggle(activeRepo) }
			} else if focus == .cancel {
				cancel()
			} else {
				create()
			}
		case KeyCode.downArrow where focus == .filter:
			cursor.move(by: 1, in: visibleRepos.count)
		case KeyCode.upArrow where focus == .filter:
			cursor.move(by: -1, in: visibleRepos.count)
		case KeyCode.space where !focus.isTextField:
			if focus == .create { create() } else { cancel() }
		default:
			return !focus.isTextField && key.isUnusedOnAPage
		}
		return true
	}

	private var trimmedName: String { name.trimmingCharacters(in: .whitespaces) }
	private var trimmedModel: String { model.trimmingCharacters(in: .whitespaces) }

	/// The form as it stands: the editable fields' values over their listed ones.
	private var currentForm: AgentCreationForm {
		let inputs = Dictionary(uniqueKeysWithValues: editableFields.map { ($0.key, value(of: $0)) })
		return AgentCreationForm(
			name: trimmedName, role: nil, repos: selectedRepoPaths, inputs: inputs, keepCodingInstructions: keepCodingInstructions,
			model: AgentModelChoice(model: trimmedModel, effort: effort))
	}

	private var selectedRepoPaths: [String] { repos.map(\.path).filter(selected.contains) }
}

struct NewAgentDialogView: View {
	@Bindable var model: NewAgentDialogModel
	@FocusState private var field: NewAgentDialogModel.Focus?
	/// The form's full height, which its scroll view grows to before it scrolls.
	@State private var formHeight: CGFloat = 0

	var body: some View {
		ZStack {
			Color.black.opacity(0.35)
				.contentShape(.rect)
				.onTapGesture { if model.run == nil { model.cancel() } }
				.accessibilityHidden(true)
			VStack(alignment: .leading, spacing: 10) {
				Text("New Agent").font(.title3.weight(.semibold))
				if let run = model.run {
					AgentCreationProgress(run: run, cancel: model.cancel)
						.frame(width: 520)
				} else {
					// The preview overlays the space beside the form, so the form alone sets the height, and the form is what narrows in a small window.
					VStack(alignment: .leading, spacing: 10) {
						ScrollView {
							// The inset keeps the fields' focus rings and hover effects clear of the scroll view's clipping.
							VStack(alignment: .leading, spacing: 10) { form }
								.padding(.horizontal, 6)
								.onGeometryChange(for: CGFloat.self, of: \.size.height) { formHeight = $0 }
						}
						.frame(maxHeight: formHeight)
						.scrollBounceBehavior(.basedOnSize)
						buttons
					}
					.frame(minWidth: 260, maxWidth: 330)
					.padding(.trailing, 500)
					.overlay(alignment: .trailing) { preview.frame(width: 480) }
				}
			}
			.padding(20)
			.background(.windowBackground, in: .rect(cornerRadius: 12))
			.shadow(radius: 24)
			.accessibilityElement(children: .contain)
			.accessibilityIdentifier(NewAgentDialogModel.layerName)
			.padding(40)
		}
		.onChange(of: model.focus, initial: true) { _, focus in field = focus.isTextField ? focus : nil }
		.onChange(of: field) { _, field in
			if let field { model.focus = field }
		}
	}

	@ViewBuilder private var form: some View {
		label("Name")
		TextField("e.g. Bravo", text: $model.name)
			.textFieldStyle(.roundedBorder)
			.focused($field, equals: .name)
			.accessibilityIdentifier("new-agent-name")
		if let spacePath = model.spacePath {
			Text("Creates \(spacePath)")
				.font(.caption)
				.foregroundStyle(.secondary)
				.lineLimit(1)
				.truncationMode(.middle)
				.textSelection(.enabled)
				.accessibilityIdentifier("new-agent-space-path")
		}
		ForEach(model.editableFields, id: \.key) { agentField in
			fieldRow(agentField)
		}
		ForEach(model.derivedFields, id: \.key) { agentField in
			HStack(alignment: .firstTextBaseline) {
				label(AgentFieldLabel.text(for: agentField.key))
				Text(agentField.value ?? "from the name")
					.font(.callout.monospaced())
					.foregroundStyle(.secondary)
					.lineLimit(2)
					.textSelection(.enabled)
			}
		}
		label("Initial repositories")
		if model.repos.isEmpty {
			Text("No repos in this profile yet.").font(.callout).foregroundStyle(.secondary)
		} else {
			TextField("Filter repos…", text: $model.filter)
				.textFieldStyle(.roundedBorder)
				.focused($field, equals: .filter)
				.accessibilityIdentifier("new-agent-filter")
			RepoListView(
				repos: model.visibleRepos, filter: model.filter, active: model.activeRepo, mode: .multi,
				isSelected: { model.selected.contains($0.path) }, click: model.click, activate: model.activate,
				identifierPrefix: "new-agent-repo-", height: 140)
		}
		label("Model")
		TextField("e.g. opus, sonnet or a full model name", text: $model.model)
			.textFieldStyle(.roundedBorder)
			.focused($field, equals: .model)
			.accessibilityIdentifier("new-agent-model")
		effortSlider
		Toggle("Include coding instructions", isOn: $model.keepCodingInstructions)
			.help("On: the instructions are appended to Claude Code's own system prompt, with its coding instructions. Off: they replace it.")
			.accessibilityIdentifier("new-agent-keep-coding-instructions")
	}

	private var buttons: some View {
		HStack(spacing: 10) {
			Spacer()
			DialogButton(title: "Cancel", isFocused: model.focus == .cancel, action: model.cancel)
			DialogButton(title: "Create", isFocused: model.focus == .create, tint: .accentColor, action: model.create)
				.disabled(!model.canCreate)
		}
		.padding(.top, 4)
	}

	/// The instructions as creation would render them, beside the form.
	private var preview: some View {
		VStack(alignment: .leading, spacing: 6) {
			label("Preview")
			ScrollView {
				switch model.preview {
				case .text(let text):
					Text(text).font(.callout.monospaced()).textSelection(.enabled)
						.frame(maxWidth: .infinity, alignment: .leading)
				case .failure(let error):
					Text(error).font(.callout).foregroundStyle(.red)
						.frame(maxWidth: .infinity, alignment: .leading)
				case nil:
					ProgressView().controlSize(.small)
				}
			}
			.padding(8)
			.background(.background.secondary, in: .rect(cornerRadius: 6))
		}
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("new-agent-preview")
	}

	/// "Claude Code default", then each effort level, as one stepped slider.
	private var effortSlider: some View {
		let levels = model.effortLevels
		let index = Binding<Double>(
			get: { Double(model.effort.flatMap { levels.firstIndex(of: $0) }.map { $0 + 1 } ?? 0) },
			set: { position in
				let step = Int(position.rounded())
				model.chooseEffort(step == 0 ? nil : levels[step - 1])
			})
		return VStack(alignment: .leading, spacing: 2) {
			HStack {
				label("Effort")
				Spacer()
				Text(model.effort ?? "Claude Code default")
					.font(.caption)
					.foregroundStyle(.secondary)
					.accessibilityIdentifier("new-agent-effort-value")
			}
			Slider(value: index, in: 0...Double(max(levels.count, 1)), step: 1)
				.disabled(levels.isEmpty)
				.accessibilityIdentifier("new-agent-effort")
				.accessibilityValue(model.effort ?? "Claude Code default")
		}
	}

	private func fieldRow(_ agentField: AgentField) -> some View {
		VStack(alignment: .leading, spacing: 4) {
			HStack {
				label(AgentFieldLabel.text(for: agentField.key))
				Spacer()
				Button(saveLabel(agentField)) { model.save(agentField) }
					.buttonStyle(.borderless)
					.font(.caption)
					.disabled(model.value(of: agentField).isEmpty || model.saved.contains(agentField.key))
					.accessibilityIdentifier("new-agent-save-\(agentField.key)")
			}
			TextField(agentField.key, text: Binding(
				get: { model.value(of: agentField) },
				set: { model.edits[agentField.key] = $0 }), axis: .vertical)
				.textFieldStyle(.roundedBorder)
				.lineLimit(1...4)
				.focused($field, equals: .field(agentField.key))
				.accessibilityIdentifier("new-agent-field-\(agentField.key)")
			Text(["{{\(agentField.key)}}", sourceLabel(agentField.source)].filter { !$0.isEmpty }.joined(separator: " · "))
				.font(.caption2.monospaced())
				.foregroundStyle(.tertiary)
		}
	}

	private func saveLabel(_ agentField: AgentField) -> String {
		if model.saved.contains(agentField.key) { return "Saved" }
		return model.saveScope(of: agentField) == .global ? "Save to global" : "Save to profile"
	}

	private func sourceLabel(_ source: AgentFieldSource?) -> String {
		switch source {
		case nil: "required"
		case .launch: "from the launch spec"
		case .default: "Devora default"
		case .global: "from global config"
		case .profile: "from profile config"
		case .derived: ""
		}
	}

	private func label(_ text: String) -> some View {
		Text(text).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
	}
}

/// The creation's steps, then its failure; Cancel during it, Close after a failure.
private struct AgentCreationProgress: View {
	let run: AgentCreationRun
	let cancel: () -> Void

	var body: some View {
		VStack(alignment: .leading, spacing: 6) {
			ForEach(Array(run.steps.enumerated()), id: \.offset) { _, step in
				Text(step).font(.callout)
			}
			switch run.outcome {
			case .failed(let message):
				Text(message).font(.callout.monospaced()).foregroundStyle(.red).textSelection(.enabled)
			case .cancelled:
				Text("Cancelled").foregroundStyle(.secondary)
			case .done, nil:
				ProgressView().controlSize(.small)
			}
			HStack {
				Spacer()
				Button(run.isRunning ? "Cancel" : "Close", action: cancel)
					.accessibilityIdentifier("new-agent-progress-action")
			}
		}
		.accessibilityIdentifier("new-agent-progress")
	}
}

/// User-facing labels for template placeholders: the built-in ones by name, any other key from its words ("repo.notes" reads "Repo notes").
enum AgentFieldLabel {
	static let known = [
		"agent.name": "Name",
		"agent.role": "Role",
		"role.details": "Role details",
		"user.details": "About you",
		"extra.details": "Extra details",
		"custom.sections": "Custom sections",
		"space.path": "Space path",
		"memory.path": "Memory path",
		"space.repos": "Repositories",
	]

	static func text(for key: String) -> String {
		if let known = known[key] { return known }
		let words = key.split(whereSeparator: { $0 == "." || $0 == "_" }).joined(separator: " ")
		return words.prefix(1).uppercased() + words.dropFirst()
	}
}
