import AppKit
import AppShell
import SwiftUI

@main
struct NvimGridSpikeApp: App {
	@NSApplicationDelegateAdaptor private var delegate: AppDelegate

	var body: some Scene {
		WindowGroup("Nvim Grid Spike") {
			AppRoot()
		}
		.restorationBehavior(.disabled)
	}
}

/// Sapphire restores its layout from core state, never from AppKit's. After an unclean exit, AppKit's restoration
/// relaunched the app with no window at all, and `.restorationBehavior(.disabled)` alone did not prevent it.
final class AppDelegate: NSObject, NSApplicationDelegate {
	func application(_ app: NSApplication, shouldSaveSecureApplicationState coder: NSCoder) -> Bool { false }
	func application(_ app: NSApplication, shouldRestoreSecureApplicationState coder: NSCoder) -> Bool { false }
	func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool { true }
}
