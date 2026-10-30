import AppKit
import AppShell
import SwiftUI

@main
struct DevoraSapphireApp: App {
	@NSApplicationDelegateAdaptor private var delegate: AppDelegate

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
