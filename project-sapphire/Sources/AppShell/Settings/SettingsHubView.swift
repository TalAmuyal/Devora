import CoreBindings
import Store
import SwiftUI

/// The Settings Hub's page: the master list beside the focused row's detail, and the key hints, as Ember's master/detail split.
struct SettingsHubView: View {
	let model: SettingsHubModel

	var body: some View {
		VStack(spacing: 0) {
			HStack(spacing: 0) {
				Text("Devora").font(.title2.weight(.semibold))
				Text(" › Settings").font(.title2).foregroundStyle(.secondary)
				Spacer()
			}
			.padding(.horizontal, 16)
			.padding(.vertical, 12)
			Divider()
			HSplitView {
				MasterList(model: model)
					.frame(minWidth: 220, idealWidth: 280, maxWidth: 380)
				DetailPanel(model: model)
					.frame(minWidth: 420, maxWidth: .infinity)
			}
			Divider()
			KeyHintBar(hints: [
				("j/k", "navigate"), ("Enter", "set active"), ("n", "new profile"), ("d", "delete"), ("q/Esc", "close"),
			])
		}
		.background(.windowBackground)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier(SettingsHubModel.layerName)
	}
}

private struct MasterList: View {
	let model: SettingsHubModel

	var body: some View {
		if !model.loaded {
			Text("Loading profiles...")
				.foregroundStyle(.secondary)
				.frame(maxWidth: .infinity, maxHeight: .infinity)
		} else {
			let rows = model.rows
			let focused = model.cursor.position(in: rows.count)
			ScrollView {
				VStack(spacing: 2) {
					ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
						MasterRow(row: row, isActive: isActive(row), isFocused: index == focused)
							.onTapGesture { model.select(row) }
					}
				}
				.padding(6)
			}
		}
	}

	private func isActive(_ row: SettingsRow) -> Bool {
		if case .profile(let profile) = row { model.isActive(profile) } else { false }
	}
}

private struct MasterRow: View {
	let row: SettingsRow
	let isActive: Bool
	let isFocused: Bool

	var body: some View {
		HStack(spacing: 8) {
			switch row {
			case .userDefaults:
				Image(systemName: "gearshape").frame(width: 12)
				Text("User Defaults")
			case .profile(let profile):
				StatusDotView(dot: isActive ? .clean : .pending)
					.frame(width: 12)
				Text(profile.name).lineLimit(1)
				Spacer(minLength: 8)
				Text(plural(Int(profile.repoCount), "repo"))
					.font(.caption)
					.foregroundStyle(.secondary)
			case .newProfile:
				Image(systemName: "plus").frame(width: 12)
				Text("New Profile…").foregroundStyle(.secondary)
			}
			Spacer(minLength: 0)
		}
		.padding(.horizontal, 10)
		.padding(.vertical, 7)
		.background(isFocused ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 6))
		.contentShape(.rect)
		.accessibilityElement(children: .combine)
		.accessibilityAddTraits(isFocused ? .isSelected : [])
	}
}

private struct DetailPanel: View {
	let model: SettingsHubModel

	var body: some View {
		if model.loaded {
			ScrollView {
				VStack(alignment: .leading, spacing: 14) {
					switch model.focusedRow {
					case .userDefaults:
						Text("User Defaults").font(.title2.weight(.semibold))
						Text("Applied to every profile unless a profile overrides a setting.").foregroundStyle(.secondary)
						if let scope = model.scope { ScopeCards(scope: scope) }
					case .profile(let profile):
						ProfileDetailView(profile: profile, model: model)
						if let scope = model.scope { ScopeCards(scope: scope) }
					case .newProfile:
						Text("New Profile").font(.title2.weight(.semibold))
						if let form = model.form { ProfileFormView(model: form) }
					}
				}
				.padding(20)
				.frame(maxWidth: 760, alignment: .leading)
				.frame(maxWidth: .infinity, alignment: .leading)
			}
		} else {
			Color.clear
		}
	}
}

private struct ProfileDetailView: View {
	let profile: Profile
	let model: SettingsHubModel

	var body: some View {
		let detail = model.details[profile.path]
		let isActive = model.isActive(profile)
		Text(profile.name).font(.title2.weight(.semibold))
		Text(profile.path).font(.callout.monospaced()).foregroundStyle(.secondary).textSelection(.enabled)
		HStack(spacing: 6) {
			if isActive { Badge(text: "active", color: .green) }
			Badge(text: plural(Int(profile.repoCount), "repo"), color: .secondary)
			if let detail {
				Badge(text: plural(detail.workspaceCount, "workspace"), color: .secondary)
				Badge(text: plural(detail.activeTaskCount, "active task"), color: .blue)
			}
		}
		HStack(spacing: 8) {
			Button("Set Active") { model.setActive(profile) }
				.buttonStyle(.borderedProminent)
				.disabled(isActive)
			Button("Clone Repo") { model.cloneRepo(into: profile) }
			Button("Delete", role: .destructive) { model.delete(profile) }
		}
		SettingsCard(title: "Repos") {
			if let detail {
				if detail.repos.isEmpty {
					Text("No repos yet — clone one into \(profile.path)/repos/").foregroundStyle(.secondary)
				} else {
					RepoGrid(repos: detail.repos)
				}
			} else {
				Text("Loading...").foregroundStyle(.secondary)
			}
		}
	}
}

private func plural(_ count: Int, _ noun: String) -> String {
	count == 1 ? "1 \(noun)" : "\(count) \(noun)s"
}

private struct RepoGrid: View {
	let repos: [RepoInfo]

	var body: some View {
		Grid(alignment: .leading, horizontalSpacing: 20, verticalSpacing: 6) {
			GridRow {
				Text("Repo")
				Text("Source")
				Text("Path")
			}
			.font(.caption.weight(.semibold))
			.foregroundStyle(.secondary)
			Divider()
			ForEach(repos, id: \.path) { repo in
				GridRow {
					Text(repo.name)
					Text(repo.source).foregroundStyle(.secondary)
					Text(repo.path).font(.callout.monospaced()).foregroundStyle(.secondary).textSelection(.enabled)
				}
			}
		}
	}
}

/// A titled card of the detail panel, as Ember's `SettingsCard`.
private struct SettingsCard<Content: View>: View {
	let title: String
	@ViewBuilder let content: Content

	var body: some View {
		VStack(alignment: .leading, spacing: 12) {
			Text(title).font(.headline)
			content
		}
		.padding(14)
		.frame(maxWidth: .infinity, alignment: .leading)
		.background(.background.secondary, in: .rect(cornerRadius: 10))
		.overlay(RoundedRectangle(cornerRadius: 10).stroke(.separator))
	}
}

/// The settings cards of one scope, which share its settings and the focus of its inputs.
private struct ScopeCards: View {
	let scope: ConfigSettingsModel
	@FocusState private var focus: String?

	var body: some View {
		ForEach(ConfigCard.all, id: \.title) { card in
			SettingsCard(title: card.title) {
				if scope.settings == nil {
					Text("Loading...").foregroundStyle(.secondary)
				} else {
					ForEach(card.fields.filter(scope.isVisible), id: \.key) { field in
						ConfigRow(field: field, scope: scope, focus: $focus)
					}
				}
			}
		}
		// On the next turn, since the input revealed with the focus is made in the same update.
		.onChange(of: scope.focusedKey, initial: true) { _, key in
			Task { @MainActor in focus = key }
		}
		.onChange(of: focus) { _, key in
			if scope.focusedKey != key { scope.focusedKey = key }
		}
	}
}

private struct ConfigRow: View {
	let field: ConfigField
	let scope: ConfigSettingsModel
	var focus: FocusState<String?>.Binding

	var body: some View {
		HStack(alignment: .top, spacing: 12) {
			VStack(alignment: .leading, spacing: 2) {
				Text(field.label)
				if let hint = field.hint {
					Text(hint).font(.caption).foregroundStyle(.secondary)
				}
			}
			.frame(width: 150, alignment: .leading)
			VStack(alignment: .leading, spacing: 6) {
				control
				if let note = field.note {
					Text(note).font(.caption).foregroundStyle(.secondary)
				}
			}
			Spacer(minLength: 0)
		}
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("config-row-\(field.key)")
	}

	@ViewBuilder private var control: some View {
		let key = field.key
		switch field.kind {
		case .text(let placeholder, let whitespace):
			let mode = scope.textMode(key)
			segments(TextMode.allCases, selection: mode, label: \.label) { scope.selectTextMode($0, for: key) }
			if mode == .set {
				HStack(spacing: 6) {
					TextField(placeholder, text: Binding(get: { scope.inputText(key) }, set: { scope.edit(key, to: $0) }))
						.textFieldStyle(.roundedBorder)
						.frame(maxWidth: 340)
						.focused(focus, equals: key)
						.onSubmit { scope.commit(key, whitespace: whitespace) }
						.accessibilityIdentifier("config-input-\(key)")
					if scope.showsSave(key, whitespace: whitespace) {
						Button { scope.commit(key, whitespace: whitespace) } label: {
							Image(systemName: "square.and.arrow.down")
						}
						.help("Save")
						.accessibilityLabel("Save")
						.accessibilityIdentifier("config-save-\(key)")
					}
				}
			} else {
				inheritedHint
			}
		case .bool:
			let choice = scope.boolChoice(key)
			segments(BoolChoice.allCases, selection: choice, label: \.label) { scope.selectBool($0, for: key) }
			if choice == .default { inheritedHint }
		case .choice(let options):
			let chosen = scope.chosenOption(options, for: key)
			segments(options, selection: chosen ?? options[0], label: \.label) { scope.selectOption($0, for: key) }
			if chosen?.value == nil { inheritedHint }
		}
	}

	@ViewBuilder private var inheritedHint: some View {
		if let hint = scope.inheritedHint(field) {
			Text(hint)
				.font(.callout)
				.foregroundStyle(.secondary)
				.accessibilityIdentifier("config-hint-\(field.key)")
		}
	}

	private func segments<Segment: Hashable>(
		_ all: [Segment], selection: Segment, label: KeyPath<Segment, String>, select: @escaping (Segment) -> Void
	) -> some View {
		Picker(field.label, selection: Binding(get: { selection }, set: { if $0 != selection { select($0) } })) {
			ForEach(all, id: \.self) { Text($0[keyPath: label]).tag($0) }
		}
		.pickerStyle(.segmented)
		.labelsHidden()
		.fixedSize()
		.accessibilityIdentifier("config-segments-\(field.key)")
	}
}
