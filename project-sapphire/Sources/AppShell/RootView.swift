import AppKit
import CoreBindings
import Layers
import Store
import SwiftUI
import TerminalKit

/// What the app does before it quits (`applicationShouldTerminate`).
public enum AppTermination {
	@MainActor public static var saveDrafts: (() -> Bool)?
	/// Whether the quit can go on now; while agents are still working it holds them, shows who it waits for, and answers false, quitting by itself once they have stopped.
	@MainActor public static var mayQuit: (() -> Bool)?
	/// Stops every agent, interrupting what still runs, and returns what went wrong; set on the main thread once the core runs, and run off the main actor.
	nonisolated(unsafe) public static var pauseAgents: (@Sendable () async -> [String])?
	/// Shows what went wrong while stopping, whose Quit then quits (`quitAfterProblems`).
	@MainActor public static var showProblems: (([String]) -> Void)?
	/// Writes a line to the app's log.
	@MainActor public static var logWarning: ((String) -> Void)?
	/// The next quit skips the wait: "Interrupt and quit".
	@MainActor public static var skipWait = false
	/// The agents are stopped already, so the next quit goes straight on.
	@MainActor public static var stopped = false

	/// "Interrupt and quit": today's quit, which cuts the agents' work short.
	@MainActor public static func interruptAndQuit() {
		skipWait = true
		NSApp.terminate(nil)
	}

	/// Quits after the problems were shown.
	@MainActor public static func quitAfterProblems() {
		NSApp.terminate(nil)
	}
}

/// The window's content: the active tab's terminal over the tab bar, under the window's surfaces (the Workspace Hub).
public struct RootView: View {
	@State private var store: Store
	@State private var terminals = Terminals()
	@State private var surfaces: Surfaces
	@State private var appearance: AppAppearance

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
		// SwiftUI builds the window's content as the app finishes launching, before it shows the window.
		let appearance = AppAppearance()
		appearance.apply(store.appearance)
		_appearance = State(initialValue: appearance)
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
				.background(KeyRouterInstaller(router: surfaces.router, shortcuts: surfaces.shortcuts))
				.background(WindowCloseQuits())
			}
		}
		.overlay(alignment: .top) { ErrorBannerStack(store: store) }
		.background(FrameDriver(store: store))
		.navigationTitle(store.windowTitle)
		.toolbar {
			if store.startupProblem == nil {
				if let indicator = surfaces.shutdownProblems != nil ? "Stopped" : store.pauseIndicator {
					// Its own item, so that the toolbar gives it its own, evenly padded capsule.
					ToolbarItem {
						Button(indicator) { surfaces.showPauseStatus() }
							.accessibilityIdentifier("global-pause-indicator")
							.help(surfaces.shutdownProblems != nil ? "Show what went wrong while stopping, and quit" : "Show who Devora is still waiting for")
							.popover(isPresented: Binding(
								get: { surfaces.pauseProgress != nil },
								set: { if !$0 { surfaces.hidePauseProgress() } }
							), arrowEdge: .bottom) {
								if let model = surfaces.pauseProgress {
									PauseProgressView(model: model, store: store, show: surfaces.showAgent(tab:))
								}
							}
					}
				}
				ToolbarItem {
					Button {
						store.dispatch(.setGlobalPaused(!store.app.globalPaused))
					} label: {
						Label(store.app.globalPaused ? "Resume Devora" : "Pause Devora", systemImage: store.app.globalPaused ? "play.fill" : "pause.fill")
					}
					.accessibilityIdentifier("global-pause-toggle")
					.help(store.app.globalPaused ? "Resume Devora" : "Pause Devora after current work finishes")
				}
			}
		}
		.onChange(of: store.app.globalPaused) { _, _ in
			surfaces.palette?.updateProfiles(store.profiles, active: store.selectedProfilePath, globalPaused: store.app.globalPaused)
			surfaces.finishPendingQuit()
		}
		.onChange(of: store.runningAgents) { _, _ in surfaces.finishPendingQuit() }
		.onChange(of: store.appearance) { _, setting in appearance.apply(setting) }
		// The terminals follow the effective appearance and the chosen palette, live.
		.onChange(of: appearance.effective, initial: true) { _, _ in applyTerminalPalette() }
		.onChange(of: store.terminalPalette) { _, _ in applyTerminalPalette() }
		.onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)) { _ in
			Task { await store.repoUpdatesWoke() }
		}
		.frame(minWidth: 900, minHeight: 560)
		.task {
			if store.startupProblem == nil {
				await launch()
			} else if TestBridge.isEnabled {
				// Early, so that the harness can see the problem and answer it.
				TestBridge.start(store: store, terminals: terminals, surfaces: surfaces, appearance: appearance)
			}
		}
		.onChange(of: store.startupProblem) { problem, resolved in
			if problem != nil, resolved == nil { Task { await launch() } }
		}
	}

	private func applyTerminalPalette() {
		let family = TerminalPalette.Family(store.terminalPalette)
		TerminalRuntime.shared.apply(light: family.light, dark: family.dark, isDark: appearance.effective == .dark)
	}

	/// Runs once the app state is open: the restored tabs are already in the core, and the Workspace Hub opens when there are none, as Ember's launch always opens it.
	private func launch() async {
		AppTermination.saveDrafts = { [weak store] in store?.flushAgentDrafts() ?? true }
		AppTermination.pauseAgents = store.agentShutdown()
		AppTermination.mayQuit = { [weak surfaces] in surfaces?.mayQuit() ?? true }
		AppTermination.showProblems = { [weak surfaces] problems in surfaces?.showShutdownProblems(problems) }
		AppTermination.logWarning = { [weak store] message in store?.logWarning(message) }
		await store.dispatch(.load).value
		await store.startRepoUpdates()
		// Restored agent tabs resume in the background, each showing its own failure.
		Task { await store.restoreAgentSpaces() }
		if let path = LaunchOptions.value("--open-workspace") {
			if let workspace = store.workspaces.first(where: { $0.path == path }) {
				store.dispatch(.openWorkspace(workspace))
			} else {
				store.showError("--open-workspace: \(path) is not a workspace of the selected profile")
			}
		}
		if store.hasNoTabs() || LaunchOptions.value("--surface") == WorkspaceHubModel.layerName {
			surfaces.openWorkspaceHub()
		}
		// Started after the launch's commands, so that the harness finds them dispatched.
		if TestBridge.isEnabled {
			TestBridge.start(store: store, terminals: terminals, surfaces: surfaces, appearance: appearance)
		}
		if let path = LaunchOptions.value("--snapshot") {
			await Snapshot.run(store: store, terminals: terminals, to: path)
		}
	}
}
