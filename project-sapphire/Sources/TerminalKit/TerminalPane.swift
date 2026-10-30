import AppKit
import SwiftUI

/// The terminal pane's single view entry point.
public struct TerminalPane: NSViewRepresentable {
	let session: TerminalSession
	let isFocused: Bool
	let router: HostKeyRouter
	let onFocus: () -> Void

	public init(session: TerminalSession, isFocused: Bool, router: @escaping HostKeyRouter, onFocus: @escaping () -> Void) {
		self.session = session
		self.isFocused = isFocused
		self.router = router
		self.onFocus = onFocus
	}

	public func makeNSView(context: Context) -> NSView {
		let view = TerminalView(session: session)
		view.router = router
		view.onFocus = onFocus
		view.wantsFocus = isFocused
		return view
	}

	public func updateNSView(_ view: NSView, context: Context) {
		guard let view = view as? TerminalView else { return }
		view.router = router
		view.onFocus = onFocus
		view.wantsFocus = isFocused
	}
}
