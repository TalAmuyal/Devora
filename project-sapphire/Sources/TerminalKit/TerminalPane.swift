import AppKit
import SwiftUI

/// The terminal pane's single view entry point; the host focuses it through `TerminalSession.focus()`.
public struct TerminalPane: NSViewRepresentable {
	let session: TerminalSession

	public init(session: TerminalSession) {
		self.session = session
	}

	public func makeNSView(context: Context) -> NSView {
		TerminalView(session: session)
	}

	public func updateNSView(_ view: NSView, context: Context) {}
}
