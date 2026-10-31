import AppKit
import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// The window's content: profiles and workspaces in the sidebar; the active tab's terminal over the tab bar in the main area.
public struct RootView: View {
	@State private var store: Store
	@State private var terminals = Terminals()
	@State private var surfaces: Surfaces

	/// Opened once per process; the app can't report anything without it, so it stops if it can't be opened, as Ember does.
	private static let sessionLog: AppLog = {
		do {
			return try AppLog.openSession()
		} catch {
			fatalError("couldn't open the log: \(Store.describe(error))")
		}
	}()

	public init() {
		let log = Self.sessionLog
		TerminalRuntime.logWarning = { message in try? log.write(level: .warn, message: message) }
		let store = Store(configPath: nil, state: .app, log: log)
		_store = State(initialValue: store)
		_surfaces = State(initialValue: Surfaces(store: store))
	}

	public var body: some View {
		Group {
			if let problem = store.startupProblem {
				StartupProblemView(problem: problem, store: store)
			} else {
				NavigationSplitView {
					Sidebar(store: store)
						.navigationSplitViewColumnWidth(min: 220, ideal: 260)
				} detail: {
					VStack(spacing: 0) {
						TabContent(store: store, terminals: terminals, surfaces: surfaces)
						TabBar(store: store)
					}
				}
				.overlay { LayerHost(stack: surfaces.windowStack) }
				.background(KeyRouterInstaller(router: surfaces.router))
			}
		}
		.overlay(alignment: .top) { ErrorBannerStack(store: store) }
		.background(FrameDriver(store: store))
		.navigationTitle(store.version.isEmpty ? "Devora Sapphire" : "Devora Sapphire \(store.version)")
		.frame(minWidth: 900, minHeight: 560)
		.task {
			if store.startupProblem == nil {
				await launch()
			} else if TestBridge.isEnabled {
				// Early, so that the harness can see the problem and answer it.
				TestBridge.start(store: store, terminals: terminals, surfaces: surfaces)
			}
		}
		.onChange(of: store.startupProblem) { problem, resolved in
			if problem != nil, resolved == nil { Task { await launch() } }
		}
	}

	/// Runs once the app state is open: the restored tabs are already in the core, so a shell tab opens only if none was restored.
	private func launch() async {
		store.dispatch(.openShellIfNoTabs)
		await store.dispatch(.load).value
		if let path = LaunchOptions.value("--open-workspace") {
			if let workspace = store.workspaces.first(where: { $0.path == path }) {
				store.dispatch(.openWorkspace(workspace))
			} else {
				store.showError("--open-workspace: \(path) is not a workspace of the selected profile")
			}
		}
		// Started after the launch's commands, so that the harness finds them dispatched.
		if TestBridge.isEnabled {
			TestBridge.start(store: store, terminals: terminals, surfaces: surfaces)
		}
		if let path = LaunchOptions.value("--snapshot") {
			await Snapshot.run(store: store, terminals: terminals, to: path)
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
