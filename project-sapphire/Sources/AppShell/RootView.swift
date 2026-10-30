import AppKit
import CoreBindings
import Store
import SwiftUI

/// The window's content: profiles and workspaces in the sidebar; the active tab's terminal over the tab bar in the main area.
public struct RootView: View {
	@State private var store = Store()
	@State private var terminals = Terminals()

	public init() {}

	public var body: some View {
		NavigationSplitView {
			Sidebar(store: store)
				.navigationSplitViewColumnWidth(min: 220, ideal: 260)
		} detail: {
			VStack(spacing: 0) {
				TabContent(store: store, terminals: terminals)
				TabBar(store: store)
			}
		}
		.background(FrameDriver(store: store))
		.navigationTitle(store.version.isEmpty ? "Devora Sapphire" : "Devora Sapphire \(store.version)")
		.frame(minWidth: 900, minHeight: 560)
		.task {
			store.dispatch(.openShell)
			await store.dispatch(.load).value
			if let path = LaunchOptions.value("--open-workspace") {
				if let workspace = store.workspaces.first(where: { $0.path == path }) {
					store.dispatch(.openWorkspace(workspace))
				} else {
					NSLog("--open-workspace: %@ is not a workspace of the selected profile", path)
				}
			}
			if let path = LaunchOptions.value("--snapshot") {
				await Snapshot.run(store: store, terminals: terminals, to: path)
			}
		}
	}
}

private struct Sidebar: View {
	let store: Store

	var body: some View {
		let selection = Binding(
			get: { store.selectedProfilePath },
			set: { path in if let path { store.dispatch(.selectProfile(path: path)) } })
		List(selection: selection) {
			Section("Profiles") {
				ForEach(store.profiles, id: \.path) { profile in
					Label(profile.name, systemImage: "person.crop.square").tag(Optional(profile.path))
				}
			}
			Section("Workspaces") {
				ForEach(store.workspaces, id: \.path) { workspace in
					Button {
						store.dispatch(.openWorkspace(workspace))
					} label: {
						WorkspaceRow(workspace: workspace)
					}
					.buttonStyle(.plain)
					// Activating an inactive workspace (a new task) comes with the Workspace Hub.
					.disabled(!workspace.active)
					.help(workspace.active ? "Open in a new tab" : "")
				}
			}
			if let error = store.error {
				Section("Error") {
					Text(error).foregroundStyle(.red).textSelection(.enabled)
				}
			}
			if !store.warnings.isEmpty {
				Section("Warnings") {
					ForEach(store.warnings, id: \.self) { Text($0).foregroundStyle(.orange) }
				}
			}
		}
		.listStyle(.sidebar)
	}
}

private struct WorkspaceRow: View {
	let workspace: Workspace

	var body: some View {
		HStack {
			Image(systemName: workspace.active ? "circle.fill" : "circle")
				.foregroundStyle(workspace.active ? .green : .secondary)
				.imageScale(.small)
			VStack(alignment: .leading) {
				Text(workspace.active ? workspace.title : "Inactive")
					.foregroundStyle(workspace.active ? .primary : .secondary)
					.lineLimit(1)
				Text(workspace.id).font(.caption).foregroundStyle(.secondary)
			}
		}
	}
}
