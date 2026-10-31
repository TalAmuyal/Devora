import AppKit
import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// The window's content: the active tab's terminal over the tab bar, under the window's surfaces (the Workspace Hub).
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
				VStack(spacing: 0) {
					TabContent(store: store, terminals: terminals, surfaces: surfaces)
					TabBar(store: store)
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
		if LaunchOptions.value("--surface") == WorkspaceHubModel.layerName {
			surfaces.openWorkspaceHub()
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
