import CoreBindings
import SwiftUI

/// What one row draws: its item, and the item's Markdown, parsed once per content.
struct TranscriptRowModel: Equatable {
	let item: TranscriptItem
	let document: MarkdownDocument?

	init(_ item: TranscriptItem, document: MarkdownDocument?) {
		self.item = item
		self.document = document
	}

	/// The text a row shows (and the test bridge reads), without its Markdown syntax where it has some.
	var plainText: String {
		switch item.content {
		case .user(let text, let error): text + (error.map { "\n\n\($0)" } ?? "")
		case .assistant(let text), .thinking(let text, _): text
		case .tool(_, let name, _, let verdict, let result):
			[name, verdict?.detail, result?.text ?? "Awaiting result…"].compactMap { $0 }.joined(separator: "\n")
		case .system(let kind, let text, _): text.isEmpty ? String(describing: kind) : text
		case .unknown(let json): json
		}
	}
}

/// One transcript item, laid out readably, with no styling of its own yet.
struct TranscriptRow: View {
	let model: TranscriptRowModel

	var body: some View {
		content
			.frame(maxWidth: .infinity, alignment: .leading)
			.padding(.horizontal, 16).padding(.vertical, 6)
			.textSelection(.enabled)
			.help("#\(model.item.sequence)")
	}

	@ViewBuilder private var content: some View {
		switch model.item.content {
		case .user(let text, let error):
			VStack(alignment: .leading, spacing: 2) {
				label("You")
				Text(text)
				if let error { Text(error).foregroundStyle(.red) }
			}
		case .assistant(let text):
			VStack(alignment: .leading, spacing: 2) {
				label("Agent")
				if let document = model.document { MarkdownView(blocks: document.blocks) } else { Text(text) }
			}
		case .thinking(let text, _):
			VStack(alignment: .leading, spacing: 2) {
				label("Thinking")
				Text(text).italic().foregroundStyle(.secondary).lineLimit(3)
			}
		case .tool(_, let name, _, let verdict, let result):
			VStack(alignment: .leading, spacing: 2) {
				HStack(spacing: 6) {
					label("Tool")
					Text(name).bold()
					if let verdict { Text(verdict.detail).font(.caption).foregroundStyle(.secondary) }
				}
				Text(result?.text ?? "Awaiting result…").font(.system(.caption, design: .monospaced)).lineLimit(6)
			}
		case .system, .unknown:
			Text(model.plainText).font(.caption).foregroundStyle(.secondary)
		}
	}

	private func label(_ text: String) -> some View {
		Text(text).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
	}
}
