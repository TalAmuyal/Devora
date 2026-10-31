import CoreBindings
import Store
import SwiftUI

/// The Workspace Hub's page: the header, the workspace list beside the focused workspace's details, and the key hints.
///
/// The header, the filter and the hints draw at once; the list shows a placeholder until the workspaces are loaded, and each status as it arrives.
struct WorkspaceHubView: View {
	@Bindable var model: WorkspaceHubModel

	var body: some View {
		VStack(spacing: 0) {
			HubHeader(model: model)
			Divider()
			HSplitView {
				WorkspaceListPanel(model: model)
					.frame(minWidth: 280, idealWidth: 340, maxWidth: 480)
				WorkspaceDetailPanel(model: model)
					.frame(minWidth: 360, maxWidth: .infinity)
			}
			Divider()
			KeyHintBar(hints: [
				("j/k", "navigate"), ("Enter", "open"), ("f", "filter"), ("1/2/3", "active/inactive/all"),
				("R", "refresh"), ("q/Esc", "close"), ("?", "all shortcuts"),
			])
			.accessibilityIdentifier("hub-hint-bar")
		}
		.background(.windowBackground)
		.overlay(alignment: .bottom) {
			if let toast = model.toast {
				Text(toast)
					.padding(.horizontal, 14)
					.padding(.vertical, 8)
					.background(.regularMaterial, in: .capsule)
					.shadow(radius: 4)
					.padding(.bottom, 48)
					.transition(.opacity)
					.accessibilityIdentifier("hub-toast")
			}
		}
		.animation(.easeInOut(duration: 0.2), value: model.toast)
	}
}

private struct HubHeader: View {
	let model: WorkspaceHubModel

	var body: some View {
		let store = model.store
		HStack {
			Text("Devora").font(.title2.weight(.semibold))
			Spacer()
			if !store.profiles.isEmpty {
				Menu {
					ForEach(store.profiles, id: \.path) { profile in
						Toggle(isOn: Binding(
							get: { profile.path == store.selectedProfilePath },
							set: { if $0 { model.selectProfile(profile.path) } })
						) {
							Text(profile.name)
							Text(profile.repoCount == 1 ? "1 repo" : "\(profile.repoCount) repos")
						}
					}
				} label: {
					Label(
						store.profiles.first { $0.path == store.selectedProfilePath }?.name ?? "Profiles",
						systemImage: "person.crop.square")
				}
				.fixedSize()
				.help("Profile")
			}
		}
		.padding(.horizontal, 16)
		.padding(.vertical, 10)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("hub-header")
	}
}

private struct WorkspaceListPanel: View {
	@Bindable var model: WorkspaceHubModel
	@FocusState private var filterFocused: Bool

	var body: some View {
		VStack(spacing: 0) {
			VStack(spacing: 8) {
				TextField("Filter…", text: $model.filter)
					.textFieldStyle(.roundedBorder)
					.focused($filterFocused)
					.accessibilityIdentifier("hub-filter")
				Picker("Category", selection: $model.category) {
					ForEach(HubCategory.allCases, id: \.self) { Text($0.label).tag($0) }
				}
				.pickerStyle(.segmented)
				.labelsHidden()
			}
			.padding(12)
			Divider()
			list
		}
		.onChange(of: model.filterFocused) { _, focused in filterFocused = focused }
		.onChange(of: filterFocused) { _, focused in model.filterFocused = focused }
	}

	@ViewBuilder private var list: some View {
		let items = model.items
		if !model.loaded {
			placeholder("Loading workspaces...")
		} else if model.store.profiles.isEmpty {
			// Creating the first profile comes with the welcome screen.
			placeholder("No profiles")
		} else if items.isEmpty {
			placeholder("No workspaces found")
		} else {
			let focusedPath = model.focused?.path
			ScrollViewReader { proxy in
				ScrollView {
					LazyVStack(spacing: 2) {
						ForEach(items, id: \.path) { workspace in
							WorkspaceRow(
								workspace: workspace, dot: workspace.dot(status: model.status(of: workspace)),
								isFocused: workspace.path == focusedPath
							)
							.id(workspace.path)
							.onTapGesture { model.select(workspace) }
						}
					}
					.padding(6)
				}
				.onChange(of: focusedPath) { _, path in
					if let path { proxy.scrollTo(path) }
				}
			}
		}
	}

	private func placeholder(_ text: String) -> some View {
		Text(text)
			.foregroundStyle(.secondary)
			.frame(maxWidth: .infinity, maxHeight: .infinity)
	}
}

private struct WorkspaceRow: View {
	let workspace: Workspace
	let dot: StatusDot
	let isFocused: Bool

	var body: some View {
		HStack(spacing: 8) {
			StatusDotView(dot: dot)
			Text(workspace.hasTask ? workspace.title : "(inactive)")
				.foregroundStyle(workspace.hasTask ? .primary : .secondary)
				.lineLimit(1)
			Spacer(minLength: 8)
			Text(workspace.id)
				.font(.caption.monospaced())
				.foregroundStyle(.secondary)
		}
		.padding(.horizontal, 10)
		.padding(.vertical, 6)
		.background(isFocused ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 6))
		.contentShape(.rect)
	}
}

struct StatusDotView: View {
	let dot: StatusDot

	var body: some View {
		Circle()
			.fill(color)
			.frame(width: 8, height: 8)
			.help(dot.rawValue)
	}

	private var color: Color {
		switch dot {
		case .clean: .green
		case .modified: .orange
		case .pending: .secondary.opacity(0.5)
		case .error: .red
		}
	}
}

private struct WorkspaceDetailPanel: View {
	let model: WorkspaceHubModel

	var body: some View {
		if model.loaded, let workspace = model.focused {
			ScrollView {
				WorkspaceDetail(workspace: workspace, status: model.status(of: workspace)) {
					model.openWorkspace(workspace)
				}
				.padding(20)
				.frame(maxWidth: .infinity, alignment: .leading)
			}
		} else {
			Color.clear
		}
	}
}

private struct WorkspaceDetail: View {
	let workspace: Workspace
	let status: WorkspaceStatus?
	let open: () -> Void

	var body: some View {
		VStack(alignment: .leading, spacing: 14) {
			Text(workspace.hasTask ? workspace.title : workspace.id)
				.font(.title2.weight(.semibold))
				.foregroundStyle(workspace.hasTask ? .primary : .secondary)
				.textSelection(.enabled)
			HStack(spacing: 10) {
				Text(workspace.id).font(.callout.monospaced()).foregroundStyle(.secondary)
				Text(workspace.repos.count == 1 ? "1 repo" : "\(workspace.repos.count) repos").foregroundStyle(.secondary)
				summary
			}
			if let error = status?.error {
				Text(error)
					.font(.callout.monospaced())
					.foregroundStyle(.red)
					.textSelection(.enabled)
					.padding(10)
					.frame(maxWidth: .infinity, alignment: .leading)
					.background(.red.opacity(0.08), in: .rect(cornerRadius: 6))
			}
			Button("Open", action: open)
				.buttonStyle(.borderedProminent)
			if status?.error == nil {
				RepoTable(names: workspace.repos, statuses: status?.repos)
			}
		}
	}

	@ViewBuilder private var summary: some View {
		if status?.error != nil {
			Badge(text: "error", color: .red)
		} else if !workspace.hasTask {
			Badge(text: "inactive", color: .secondary)
		} else if let status {
			if status.hasChanges {
				Text([status.modified > 0 ? "\(status.modified) modified" : nil, status.untracked > 0 ? "\(status.untracked) untracked" : nil]
					.compactMap(\.self).joined(separator: ", "))
					.foregroundStyle(.orange)
			} else {
				Text("all clean").foregroundStyle(.green)
			}
		}
	}
}

/// The workspace's repos with their branch and changes; until the statuses arrive, each repo is pending.
private struct RepoTable: View {
	let names: [String]
	let statuses: [RepoStatus]?

	var body: some View {
		Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 8) {
			GridRow {
				Text("Repo")
				Text("Branch")
				Text("Status")
			}
			.font(.caption.weight(.semibold))
			.foregroundStyle(.secondary)
			Divider()
			if let statuses {
				ForEach(statuses, id: \.name) { repo in
					GridRow {
						Text(repo.name)
						Text(repo.branch)
							.font(.body.monospaced())
							.italic(repo.isDetached)
							.foregroundStyle(repo.isDetached ? .secondary : .primary)
							.help(repo.isDetached ? "Detached HEAD" : "")
						HStack(spacing: 6) {
							if repo.modified == 0 && repo.untracked == 0 {
								Badge(text: "✓ clean", color: .green)
							}
							if repo.modified > 0 { Badge(text: "~\(repo.modified) modified", color: .orange) }
							if repo.untracked > 0 { Badge(text: "+\(repo.untracked) untracked", color: .blue) }
						}
					}
				}
			} else {
				ForEach(names, id: \.self) { name in
					GridRow {
						Text(name)
						Text("—").foregroundStyle(.secondary)
						Badge(text: "pending", color: .secondary)
					}
				}
			}
		}
	}
}

private struct Badge: View {
	let text: String
	let color: Color

	var body: some View {
		Text(text)
			.font(.caption)
			.padding(.horizontal, 6)
			.padding(.vertical, 2)
			.foregroundStyle(color)
			.background(color.opacity(0.12), in: .capsule)
	}
}

/// The key hints along the bottom of a page, as Ember's `KeyboardHintBar`.
struct KeyHintBar: View {
	let hints: [(keys: String, description: String)]

	var body: some View {
		HStack(spacing: 14) {
			ForEach(hints, id: \.keys) { hint in
				HStack(spacing: 4) {
					KeyCap(hint.keys)
					Text(hint.description).foregroundStyle(.secondary)
				}
			}
			Spacer(minLength: 0)
		}
		.font(.caption)
		.padding(.horizontal, 16)
		.padding(.vertical, 8)
		.background(.bar)
	}
}

struct KeyCap: View {
	let keys: String

	init(_ keys: String) {
		self.keys = keys
	}

	var body: some View {
		Text(keys)
			.font(.caption.monospaced())
			.padding(.horizontal, 5)
			.padding(.vertical, 1)
			.background(.quaternary, in: .rect(cornerRadius: 4))
	}
}
