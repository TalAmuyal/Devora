import AppKit
import SwiftUI

/// Closing the window quits the app, so a close goes through the quit (`applicationShouldTerminate`) instead of closing the window first:
/// a quit that has to wait for the agents then still has its window to show who it waits for.
///
/// SwiftUI owns the window's delegate, so this one stands in front of it, answering `windowShouldClose` and forwarding everything else.
struct WindowCloseQuits: NSViewRepresentable {
	func makeNSView(context: Context) -> InstallerView { InstallerView() }
	func updateNSView(_ view: InstallerView, context: Context) { view.install() }

	final class InstallerView: NSView {
		private let delegate = QuittingWindowDelegate()

		override func viewDidMoveToWindow() {
			super.viewDidMoveToWindow()
			install()
		}

		/// Again on every update, in case SwiftUI has set its own delegate since.
		func install() {
			guard let window, window.delegate !== delegate else { return }
			delegate.original = window.delegate
			window.delegate = delegate
		}
	}
}

final class QuittingWindowDelegate: NSObject, NSWindowDelegate {
	weak var original: (any NSWindowDelegate)?

	func windowShouldClose(_ sender: NSWindow) -> Bool {
		NSApp.terminate(nil)
		return false
	}

	override func responds(to selector: Selector!) -> Bool {
		super.responds(to: selector) || original?.responds(to: selector) == true
	}

	override func forwardingTarget(for selector: Selector!) -> Any? {
		original?.responds(to: selector) == true ? original : super.forwardingTarget(for: selector)
	}
}
