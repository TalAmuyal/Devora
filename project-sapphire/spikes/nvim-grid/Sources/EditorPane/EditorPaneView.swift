import AppKit
import SwiftUI

/// The editor pane's single view entry point: the native grid plus native cmdline, popup menu and messages.
public struct EditorPaneView: View {
	let session: EditorSession
	let latency: LatencyRecorder
	let isFocused: Bool
	let onFocus: () -> Void
	private let cell = Fonts(size: 13).cell

	public init(session: EditorSession, latency: LatencyRecorder, isFocused: Bool, onFocus: @escaping () -> Void) {
		self.session = session
		self.latency = latency
		self.isFocused = isFocused
		self.onFocus = onFocus
	}

	public var body: some View {
		ZStack(alignment: .topLeading) {
			GridRepresentable(session: session, latency: latency, isFocused: isFocused, onFocus: onFocus)
			if let popup = session.model.popup {
				PopupMenu(popup: popup)
					.offset(x: CGFloat(popup.col) * cell.width, y: CGFloat(popup.row + 1) * cell.height)
			}
			VStack(alignment: .leading, spacing: 6) {
				Spacer()
				MessageList(messages: session.model.messages)
				if let cmdline = session.model.cmdline {
					CmdlineBar(cmdline: cmdline)
				}
			}
			.padding(8)
		}
	}
}

private struct GridRepresentable: NSViewRepresentable {
	let session: EditorSession
	let latency: LatencyRecorder
	let isFocused: Bool
	let onFocus: () -> Void

	func makeNSView(context: Context) -> GridView {
		let view = GridView(session: session, latency: latency)
		view.onFocusChange = { focused in if focused { onFocus() } }
		return view
	}

	func updateNSView(_ view: GridView, context: Context) {
		if isFocused, view.window?.firstResponder !== view {
			view.window?.makeFirstResponder(view)
		}
	}
}

/// `ext_cmdline`: a native command line instead of Neovim's last grid row.
private struct CmdlineBar: View {
	let cmdline: CmdlineState

	var body: some View {
		let utf8 = Array(cmdline.text.utf8)
		let pos = min(cmdline.pos, utf8.count)
		let before = String(decoding: utf8[..<pos], as: UTF8.self)
		let after = String(decoding: utf8[pos...], as: UTF8.self)
		HStack(spacing: 0) {
			Text(cmdline.firstc + cmdline.prompt + before)
			Rectangle().frame(width: 2, height: 16).foregroundStyle(.tint)
			Text((cmdline.specialChar ?? "") + after)
			Spacer(minLength: 0)
		}
		.font(.body.monospaced())
		.padding(.horizontal, 10)
		.padding(.vertical, 6)
		.background(.regularMaterial, in: .rect(cornerRadius: 8))
		.shadow(radius: 4)
	}
}

/// `ext_popupmenu`: a native completion list anchored under the cursor.
private struct PopupMenu: View {
	let popup: PopupState

	var body: some View {
		VStack(alignment: .leading, spacing: 0) {
			ForEach(popup.items.prefix(12)) { item in
				HStack {
					Text(item.word).font(.body.monospaced())
					Spacer(minLength: 16)
					Text(item.kind.isEmpty ? item.menu : item.kind).foregroundStyle(.secondary).font(.caption)
				}
				.padding(.horizontal, 8)
				.padding(.vertical, 2)
				.background(item.id == popup.selected ? Color.accentColor.opacity(0.35) : .clear)
			}
		}
		.frame(minWidth: 180, alignment: .leading)
		.padding(.vertical, 4)
		.background(.regularMaterial, in: .rect(cornerRadius: 6))
		.shadow(radius: 6)
		.fixedSize()
	}
}

/// `ext_messages`: messages as native notices rather than grid rows.
private struct MessageList: View {
	let messages: [MessageLine]

	var body: some View {
		ForEach(messages.suffix(4)) { message in
			Text(message.text)
				.font(.callout.monospaced())
				.foregroundStyle(message.kind.contains("err") ? .red : .primary)
				.padding(.horizontal, 10)
				.padding(.vertical, 4)
				.background(.thinMaterial, in: .rect(cornerRadius: 6))
		}
	}
}
