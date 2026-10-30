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
		WindowGroup("Devora Sapphire") {
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
}
