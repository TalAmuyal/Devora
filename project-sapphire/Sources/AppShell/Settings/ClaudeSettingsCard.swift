import SwiftUI

/// Claude's six settings use the core's metadata for all suggestion/effort choices.
struct ClaudeSettingsCard: View {
	let model: ClaudeSettingsModel
	@FocusState private var focus: String?

	var body: some View {
		SettingsCard(title: "Claude Launch Settings") {
			if model.settings == nil || model.choices == nil {
				Text("Loading…").foregroundStyle(.secondary)
			} else {
				combo("opus-model", "Opus tier", "ANTHROPIC_DEFAULT_OPUS_MODEL")
				combo("sonnet-model", "Sonnet tier", "ANTHROPIC_DEFAULT_SONNET_MODEL")
				combo("haiku-model", "Haiku tier", "ANTHROPIC_DEFAULT_HAIKU_MODEL")
				row("Effort", "--effort") {
					segments("effort", ["Default"] + Array((model.choices?.effortLevels ?? []).reversed()) + ["None"], selected: model.effort) {
						model.selectEffort($0)
					}
					if model.effort == "Default" { hint("effort") }
					if model.effort == "None" { Text("No override — Claude Code decides").foregroundStyle(.secondary) }
					if !(["Default", "None"] + (model.choices?.effortLevels ?? [])).contains(model.effort) {
						Text("→ \(model.effort)").foregroundStyle(.secondary)
					}
				}
				row("Default model", "ANTHROPIC_MODEL") {
					segments("default-model", ["On", "Off"], selected: model.defaultModelOn ? "On" : "Off") {
						model.setDefaultModel($0 == "On")
					}
					Text(model.defaultModelOn ? "→ opusplan" : "→ not set").foregroundStyle(.secondary)
				}
				combo("permission-mode", "Permission mode", "--permission-mode")
			}
		}
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("claude-settings-card")
		.onChange(of: model.focusedKey, initial: true) { _, key in
			Task { @MainActor in focus = key }
		}
		.onChange(of: focus) { _, key in model.focusedKey = key }
	}

	private func combo(_ key: String, _ label: String, _ detail: String) -> some View {
		row(label, detail) {
			let mode = model.mode(key)
			segments(key, ClaudeMode.allCases.map(\.label), selected: mode.label) { label in
				if let mode = ClaudeMode.allCases.first(where: { $0.label == label }) { model.select(mode, key: key) }
			}
			if mode == .custom {
				HStack(spacing: 6) {
					TextField(key == "permission-mode" ? "permission mode, then Enter" : "model id, then Enter", text: Binding(get: { model.input(key) }, set: { model.edit(key, $0) }))
						.textFieldStyle(.roundedBorder)
						.frame(maxWidth: 340)
						.focused($focus, equals: key)
						.onSubmit { model.commit(key) }
						.accessibilityIdentifier("claude-input-\(key)")
					if model.showsSave(key) {
						Button { model.commit(key); focus = key } label: { Image(systemName: "square.and.arrow.down") }
							.help("Save")
							.accessibilityLabel("Save")
							.accessibilityIdentifier("claude-save-\(key)")
					}
				}
				ViewThatFits(in: .horizontal) {
					HStack(spacing: 6) {
						ForEach(suggestions(key), id: \.self) { suggestion in suggestionButton(key, suggestion) }
					}
					VStack(alignment: .leading, spacing: 6) {
						ForEach(suggestions(key), id: \.self) { suggestion in suggestionButton(key, suggestion) }
					}
				}
			} else if mode == .default {
				hint(key)
			} else {
				Text("No override — Claude Code decides").foregroundStyle(.secondary)
			}
		}
	}

	private func suggestions(_ key: String) -> [String] {
		key == "permission-mode" ? model.choices?.permissionModeSuggestions ?? [] : model.choices?.modelSuggestions ?? []
	}

	private func suggestionButton(_ key: String, _ suggestion: String) -> some View {
		Button(suggestion) { model.edit(key, suggestion); model.commit(key); focus = key }
			.font(.caption)
			.accessibilityIdentifier("claude-suggestion-\(key)-\(suggestion)")
	}

	private func hint(_ key: String) -> some View {
		Text(model.inheritedHint(key)).foregroundStyle(.secondary).accessibilityIdentifier("claude-hint-\(key)")
	}

	private func row<Content: View>(_ label: String, _ detail: String, @ViewBuilder content: () -> Content) -> some View {
		VStack(alignment: .leading, spacing: 6) {
			VStack(alignment: .leading, spacing: 2) {
				Text(label)
				Text(detail).font(.caption).foregroundStyle(.secondary)
			}
			VStack(alignment: .leading, spacing: 6, content: content)
		}
		.frame(maxWidth: .infinity, alignment: .leading)
	}

	private func segments(_ key: String, _ labels: [String], selected: String, select: @escaping (String) -> Void) -> some View {
		Picker(key, selection: Binding(get: { selected }, set: { if $0 != selected { select($0) } })) {
			ForEach(labels, id: \.self) { Text($0).tag($0) }
		}
		.pickerStyle(.segmented).labelsHidden()
		.accessibilityIdentifier("claude-segments-\(key)")
	}
}

struct AsanaTokenRow: View {
	@Bindable var model: AsanaTokenModel

	var body: some View {
		VStack(alignment: .leading, spacing: 6) {
			VStack(alignment: .leading, spacing: 2) {
				Text("Asana API token")
				Text(model.isSet.map { $0 ? "Set" : "Not set" } ?? "Loading…")
					.font(.caption).foregroundStyle(.secondary)
					.accessibilityIdentifier("asana-token-status")
			}
			VStack(alignment: .leading, spacing: 6) {
				HStack {
					SecureField("API token", text: $model.draft)
						.textFieldStyle(.roundedBorder).frame(maxWidth: 340)
						.onSubmit { model.saveToken() }
						.accessibilityIdentifier("asana-token-input")
					Button("Save") { model.saveToken() }.disabled(!model.canSave)
						.accessibilityIdentifier("asana-token-save")
					Button("Clear") { model.clearToken() }.disabled(model.isSet != true)
						.accessibilityIdentifier("asana-token-clear")
				}
				Text("Stored in your macOS keychain and shared with Debi.").font(.caption).foregroundStyle(.secondary)
			}
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("asana-token-row")
	}
}
