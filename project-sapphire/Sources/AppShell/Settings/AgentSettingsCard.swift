import AppKit
import CoreBindings
import SwiftUI

/// Additional Agent controls inside the scope's existing Agent card.
struct AgentSettingsFields: View {
	let model: AgentSettingsModel
	@FocusState private var focus: String?

	var body: some View {
		Group {
			if model.settings == nil {
				Text("Loading agent settings…").foregroundStyle(.secondary)
			} else {
				ForEach(AgentSettingsField.all) { field in
					VStack(alignment: .leading, spacing: 6) {
						Text(field.label)
						Picker(field.label, selection: Binding(
							get: { model.mode(field.key) }, set: { model.select($0, key: field.key) })) {
							ForEach(TextMode.allCases, id: \.self) { mode in Text(mode.label).tag(mode) }
						}
						.pickerStyle(.segmented).labelsHidden().frame(maxWidth: 220)
						.accessibilityIdentifier("agent-mode-\(field.key)")
						if model.mode(field.key) == .set {
							fieldEditor(field)
							if case .cleared? = model.entry(field.key)?.stored {
								Text("Explicitly cleared in this config. Default removes the override.")
									.font(.caption).foregroundStyle(.secondary)
							}
						}
						if let entry = model.entry(field.key) {
							Text(model.hint(entry)).font(.caption).foregroundStyle(.secondary)
								.textSelection(.enabled).lineLimit(3)
								.accessibilityIdentifier("agent-hint-\(field.key)")
						}
					}
					.accessibilityElement(children: .contain)
					.accessibilityIdentifier("agent-setting-\(field.key)")
				}
				Divider()
				inputs
			}
		}
		.onChange(of: model.focusedKey) { _, key in focus = key }
		.onChange(of: focus) { _, key in model.focusedKey = key }
	}

	@ViewBuilder private func fieldEditor(_ field: AgentSettingsField) -> some View {
		let text = Binding(get: { model.input(field.key) }, set: { model.edit(field.key, $0) })
		if field.multiline {
			TextEditor(text: text)
				.font(.body).frame(height: 85).border(.separator)
				.focused($focus, equals: field.key)
				.accessibilityIdentifier("agent-input-\(field.key)")
		} else {
			HStack {
				TextField(field.key == "compact-threshold" ? "1–100" : "Template path", text: text)
					.textFieldStyle(.roundedBorder).focused($focus, equals: field.key)
					.onSubmit { model.commit(field.key) }
					.accessibilityIdentifier("agent-input-\(field.key)")
				if field.key == "template-file" {
					Button("Browse…") {
						Task {
							if let path = await AgentTemplatePicker.choose() { model.edit(field.key, path) }
						}
					}
					.accessibilityIdentifier("agent-template-browse")
				}
			}
		}
		if model.showsSave(field) {
			Button("Save") { model.commit(field.key) }
				.accessibilityIdentifier("agent-save-\(field.key)")
		}
	}

	private var inputs: some View {
		VStack(alignment: .leading, spacing: 10) {
			Text("Template inputs").font(.headline)
			Text("Keys use letters, digits, dots and underscores. Role, user details and custom sections have their own fields.")
				.font(.caption).foregroundStyle(.secondary)
			if model.settings?.inputsCleared == true {
				Text("This config explicitly clears inherited template inputs.")
				Button("Default") { model.resetInputs() }
					.accessibilityIdentifier("agent-inputs-default")
			} else {
				ForEach(model.settings?.inputs ?? [], id: \.key) { entry in inputRow(entry) }
				VStack(alignment: .leading, spacing: 6) {
					TextField("New input key", text: Binding(get: { model.newInput.key }, set: { model.newInput.key = $0 }))
						.textFieldStyle(.roundedBorder).accessibilityIdentifier("agent-new-input-key")
					TextEditor(text: Binding(get: { model.newInput.value }, set: { model.newInput.value = $0 }))
						.frame(height: 65).border(.separator).accessibilityIdentifier("agent-new-input-value")
					Button("Add input") { model.addInput() }
						.disabled(model.newInput.key.isEmpty).accessibilityIdentifier("agent-add-input")
				}
				.disabled(model.savingInputs.contains("$new"))
			}
		}
	}

	private func inputRow(_ entry: AgentSettingEntry) -> some View {
		VStack(alignment: .leading, spacing: 6) {
			if entry.stored != nil {
				TextField("Input key", text: Binding(
					get: { model.inputDraft(entry).key }, set: { model.editInput(entry, key: $0) }))
					.textFieldStyle(.roundedBorder).accessibilityIdentifier("agent-map-key-\(entry.key)")
			} else {
				Text(entry.key).font(.body.monospaced())
			}
			TextEditor(text: Binding(
				get: { model.inputDraft(entry).value }, set: { model.editInput(entry, value: $0) }))
				.frame(height: 65).border(.separator).accessibilityIdentifier("agent-map-value-\(entry.key)")
			HStack {
				Button("Save") { model.saveInput(entry) }.accessibilityIdentifier("agent-map-save-\(entry.key)")
				if entry.stored != nil {
					Button("Default") { model.defaultInput(entry) }.accessibilityIdentifier("agent-map-default-\(entry.key)")
				}
			}
			Text(model.hint(entry)).font(.caption).foregroundStyle(.secondary).lineLimit(3)
		}
		.disabled(model.savingInputs.contains(entry.key))
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("agent-map-row-\(entry.key)")
	}
}

enum AgentTemplatePicker {
	@MainActor
	static func choose() async -> String? {
		let panel = NSOpenPanel()
		panel.canChooseDirectories = false
		panel.canChooseFiles = true
		panel.allowsMultipleSelection = false
		panel.prompt = "Choose Template"
		let response = if let window = NSApp.keyWindow {
			await panel.beginSheetModal(for: window)
		} else {
			panel.runModal()
		}
		return response == .OK ? panel.url?.path : nil
	}
}
