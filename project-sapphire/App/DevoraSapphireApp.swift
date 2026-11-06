import AppKit
import AppShell
import SwiftUI

@main
struct DevoraSapphireApp: App {
	@NSApplicationDelegateAdaptor private var delegate: AppDelegate

	init() {
		// AppKit still saves its window state while the app runs, and only drops it on a clean quit; after a crash or a kill, it found that state at launch, restored no window (restoration is disabled) and so opened none.
		// This is the flag Xcode's "launch without state restoration" sets: AppKit ignores whatever state it saved.
		UserDefaults.standard.register(defaults: ["ApplePersistenceIgnoreState": true])
	}

	var body: some Scene {
		// One window: it renders the core's one window state, and its store is the app state's only writer.
		Window("Devora Sapphire", id: "main") {
			RootView()
		}
		.restorationBehavior(.disabled)
	}
}

/// Sapphire restores its layout from core state, never from AppKit's.
final class AppDelegate: NSObject, NSApplicationDelegate {
	func application(_ app: NSApplication, shouldSaveSecureApplicationState coder: NSCoder) -> Bool { false }
	func application(_ app: NSApplication, shouldRestoreSecureApplicationState coder: NSCoder) -> Bool { false }
	func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool { true }
	/// Closing the window quits, keeping the tabs for the next launch, as Cmd+Q does.
	func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
	/// Quitting pauses every agent first, so each saves its session and resumes on the next launch (ADR-012).
	///
	/// `terminate` may be called from a block on the main queue (the test bridge's quit), and AppKit then waits in a nested run loop inside that block, which can't drain the main queue, so neither the pause nor the reply may need it: the pause runs off the main actor, and the reply is a run-loop block.
	func applicationShouldTerminate(_ app: NSApplication) -> NSApplication.TerminateReply {
		if AppTermination.saveDrafts?() == false { return .terminateCancel }
		guard let pauseAgents = AppTermination.pauseAgents else { return .terminateNow }
		Task.detached {
			await pauseAgents()
			let main = CFRunLoopGetMain()
			CFRunLoopPerformBlock(main, CFRunLoopMode.commonModes.rawValue) {
				MainActor.assumeIsolated { NSApp.reply(toApplicationShouldTerminate: true) }
			}
			CFRunLoopWakeUp(main)
		}
		return .terminateLater
	}
}
