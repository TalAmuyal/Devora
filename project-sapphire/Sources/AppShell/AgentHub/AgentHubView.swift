import CoreBindings
import Store
import SwiftUI

/// The Agent Hub's page: the header with the hub switcher, the agent list beside the focused agent's details, and the key hints.
struct AgentHubView: View {
	let model: AgentHubModel

	var body: some View {
		VStack(spacing: 0) {
			HStack {
				Text("Devora").font(.title2.weight(.semibold))
				Spacer()
				HubSwitcher(current: .agents, switchHub: model.switchHub)
				Spacer()
				Text(model.profileName ?? "No profile").foregroundStyle(.secondary)
			}
			.padding(.horizontal, 16)
			.padding(.vertical, 10)
			Divider()
			if model.store.selectedProfilePath == nil {
				placeholder("Register a profile in the Workspace Hub (⌃S) to create agents.")
			} else {
				HSplitView {
					list.frame(minWidth: 280, idealWidth: 340, maxWidth: 480)
					AgentDetail(model: model).frame(minWidth: 360, maxWidth: .infinity)
				}
			}
			Divider()
			KeyHintBar(hints: [
				("j/k", "navigate"), ("Enter", "open"), ("n", "new agent"), ("d", "delete"), ("R", "refresh"),
				("⌃S", "workspaces"), ("q/Esc", "close"),
			])
		}
		.background(.windowBackground)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier(AgentHubModel.layerName)
	}

	@ViewBuilder private var list: some View {
		VStack(spacing: 0) {
			HStack {
				Text("Agents").font(.headline)
				Spacer()
				Button("+ New Agent", action: model.newAgent)
					.accessibilityIdentifier("agent-hub-new")
			}
			.padding(12)
			Divider()
			let items = model.items
			if !model.loaded {
				placeholder("Loading agents...")
			} else if items.isEmpty {
				placeholder("No agents yet — press n to create one.")
			} else {
				let focused = model.focused?.name
				ScrollView {
					LazyVStack(spacing: 2) {
						ForEach(items, id: \.name) { space in
							AgentRow(space: space, isFocused: space.name == focused)
								.onTapGesture { model.select(space) }
						}
					}
					.padding(6)
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

private struct AgentRow: View {
	let space: AgentSpace
	let isFocused: Bool

	var body: some View {
		HStack(spacing: 8) {
			Text(space.name).lineLimit(1)
			Spacer(minLength: 8)
			Text(space.error == nil ? space.role ?? "" : "invalid")
				.font(.caption)
				.foregroundStyle(space.error == nil ? Color.secondary : .red)
				.lineLimit(1)
		}
		.padding(.horizontal, 10)
		.padding(.vertical, 6)
		.background(isFocused ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 6))
		.contentShape(.rect)
	}
}

private struct AgentDetail: View {
	let model: AgentHubModel

	var body: some View {
		if model.loaded, let space = model.focused {
			ScrollView {
				VStack(alignment: .leading, spacing: 14) {
					Text(space.name).font(.title2.weight(.semibold)).textSelection(.enabled)
					if let role = space.role { Text(role).foregroundStyle(.secondary) }
					Text(space.path).font(.callout.monospaced()).foregroundStyle(.secondary).textSelection(.enabled)
					if let error = space.error {
						Text(error).font(.callout.monospaced()).foregroundStyle(.red).textSelection(.enabled)
					}
					HStack {
						if space.error == nil {
							Button("Open") { model.open(space) }
						}
						Button("Delete", role: .destructive) { model.delete(space) }
					}
				}
				.padding(20)
				.frame(maxWidth: .infinity, alignment: .leading)
			}
		} else {
			Color.clear
		}
	}
}

/// The Agents | Workspaces toggle both hubs draw in their header; it does what `Ctrl+S` does inside a hub.
struct HubSwitcher: View {
	enum Hub { case agents, workspaces }
	let current: Hub
	let switchHub: () -> Void

	var body: some View {
		Picker("Hub", selection: Binding(get: { current }, set: { if $0 != current { switchHub() } })) {
			Text("Agents").tag(Hub.agents)
			Text("Workspaces").tag(Hub.workspaces)
		}
		.pickerStyle(.segmented)
		.labelsHidden()
		.fixedSize()
		.help("Switch hubs (⌃S)")
		.accessibilityIdentifier("hub-switcher")
	}
}
