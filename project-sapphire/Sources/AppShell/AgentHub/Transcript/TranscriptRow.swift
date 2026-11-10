import AppKit
import CoreBindings
import SwiftUI

/// What one row draws: its item, the item's Markdown (parsed once per content), and whether a thinking or tool row is
/// expanded.
struct TranscriptRowModel: Equatable {
	let item: TranscriptItem
	let document: MarkdownDocument?
	let expanded: Bool

	init(_ item: TranscriptItem, document: MarkdownDocument?, expanded: Bool = false) {
		self.item = item
		self.document = document
		self.expanded = expanded
	}

	enum Kind: String {
		case user, assistant, thinking, tool, system
	}

	var kind: Kind {
		switch item.content {
		case .user: .user
		case .assistant: .assistant
		case .thinking: .thinking
		case .tool: .tool
		case .system, .unknown: .system
		}
	}

	/// Thinking and tool rows show a summary, and their body only when expanded.
	var collapsible: Bool { kind == .thinking || kind == .tool }

	/// The Markdown a row renders: a user's prompt, a reply or a thought. None for the rest.
	static func markdown(of content: TranscriptContent) -> String? {
		switch content {
		case .user(let text, _), .assistant(let text), .thinking(let text, _): text
		case .tool, .system, .unknown: nil
		}
	}

	/// The text a row has (and the test bridge reads), collapsed or not, as the web transcript's row text.
	var plainText: String {
		switch item.content {
		case .user(let text, let error): text + (error.map { "\n\n\($0)" } ?? "")
		case .assistant(let text): text
		case .thinking(let text, _): "Thinking\n\(text)"
		case .tool(_, let name, let input, let verdict, let result):
			[name, verdict.map { "Judgess \($0.kind)" }, prettyJSON(input), result?.text ?? "Awaiting result…"]
				.compactMap { $0 }.joined(separator: "\n")
		case .system(let kind, let text, _): text.isEmpty ? String(describing: kind) : text
		case .unknown(let json): prettyJSON(json)
		}
	}

	/// What Copy puts on the pasteboard: the item as written, its Markdown included, rather than as drawn.
	var source: String {
		switch item.content {
		case .user(let text, _), .assistant(let text), .thinking(let text, _): text
		case .tool(_, let name, let input, _, let result): [name, prettyJSON(input), result?.text].compactMap { $0 }.joined(separator: "\n\n")
		case .system, .unknown: plainText
		}
	}

	/// What VoiceOver says first: the row's kind, and its state where it has one.
	var accessibilityLabel: String {
		var parts: [String]
		switch item.content {
		case .user: parts = ["You"]
		case .assistant: parts = ["Agent"]
		case .thinking: parts = ["Thinking"]
		case .tool(_, let name, _, let verdict, _): parts = ["Tool \(name)"] + (verdict.map { ["Judgess \($0.kind)"] } ?? [])
		case .system, .unknown: parts = ["System"]
		}
		if collapsible { parts.append(expanded ? "expanded" : "collapsed") }
		if item.state == .interrupted { parts.append("interrupted") }
		if item.state == .failed { parts.append("failed") }
		return parts.joined(separator: ", ")
	}

	/// What VoiceOver reads after the kind: the text, without what a collapsed row hides.
	var accessibilityValue: String {
		switch item.content {
		case .thinking(let text, _): expanded ? text : ""
		case .tool(_, _, let input, _, let result): expanded ? [prettyJSON(input), result?.text ?? "Awaiting result…"].joined(separator: "\n") : ""
		default: plainText
		}
	}
}

/// One transcript item, as the web transcript draws it, with no bubbles yet.
struct TranscriptRow: View {
	let model: TranscriptRowModel
	/// Expands or collapses a thinking or tool row.
	var toggle: () -> Void = {}

	var body: some View {
		content
			.frame(maxWidth: .infinity, alignment: .leading)
			.kindMarker(model.kind, unfinished: model.item.state == .interrupted || model.item.state == .failed)
			.padding(.horizontal, 16).padding(.vertical, 6)
			.textSelection(.enabled)
			.help("#\(model.item.sequence)")
			.environment(\.openURL, OpenURLAction { Self.opensLink($0) ? .systemAction : .discarded })
			.contextMenu {
				Button("Copy") {
					NSPasteboard.general.clearContents()
					NSPasteboard.general.setString(model.source, forType: .string)
				}
			}
			// One element per row, in reading order: its kind, then its text (ADR-014's accessibility).
			.accessibilityElement(children: .ignore)
			.accessibilityLabel(model.accessibilityLabel)
			.accessibilityValue(model.accessibilityValue)
			// Without a role that has a value, macOS drops the value and VoiceOver reads only the kind.
			.accessibilityAddTraits(.isStaticText)
			.accessibilityActions {
				if model.collapsible { Button(model.expanded ? "Collapse" : "Expand", action: toggle) }
				ForEach(Array(links.enumerated()), id: \.offset) { _, link in
					Button("Open \(link.text)") { NSWorkspace.shared.open(link.url) }
				}
			}
	}

	@ViewBuilder private var content: some View {
		switch model.item.content {
		case .user(let text, let error):
			VStack(alignment: .leading, spacing: 4) {
				markdown(text)
				if let error { Text(error).foregroundStyle(.red) }
			}
		case .assistant(let text):
			markdown(text)
		case .thinking(let text, _):
			VStack(alignment: .leading, spacing: 4) {
				summary { Text("Thinking") }
				if model.expanded { markdown(text).foregroundStyle(.secondary) }
			}
		case .tool(_, let name, let input, let verdict, let result):
			VStack(alignment: .leading, spacing: 4) {
				summary {
					HStack(spacing: 6) {
						Text(name)
						if let verdict { VerdictBadge(verdict: verdict, sequence: model.item.sequence) }
					}
				}
				if model.expanded {
					Group {
						Text(prettyJSON(input))
						Text(result?.text ?? "Awaiting result…")
					}
					.font(.system(.callout, design: .monospaced))
				}
			}
		case .system, .unknown:
			Text(model.plainText).font(.callout).foregroundStyle(.secondary)
		}
	}

	/// A row's Markdown, from its parsed document (`fallback` only before one exists).
	@ViewBuilder private func markdown(_ fallback: String) -> some View {
		if let document = model.document { MarkdownView(blocks: document.blocks) } else { Text(fallback) }
	}

	/// The web transcript's `<summary>`: a disclosure triangle and the label, which toggle the row.
	private func summary(@ViewBuilder _ label: () -> some View) -> some View {
		Button(action: toggle) {
			HStack(spacing: 4) {
				Image(systemName: model.expanded ? "chevron.down" : "chevron.right").font(.caption2)
				label()
			}
			.foregroundStyle(.secondary)
			.contentShape(Rectangle())
		}
		.buttonStyle(.plain)
	}

	/// The links in the row's Markdown, each an accessibility action, since the row is one element.
	private var links: [(text: String, url: URL)] {
		model.document?.links.filter { Self.opensLink($0.url) } ?? []
	}

	/// The agent's output is untrusted (it can repeat what a fetched page injected), so a link opens only on the web, as
	/// in the web transcript: never a file, a settings pane or another app's scheme.
	static func opensLink(_ url: URL) -> Bool {
		["http", "https"].contains(url.scheme?.lowercased() ?? "")
	}
}

/// The web transcript's Judgess badge: the verdict's kind in its color, with the sequence and detail as its tooltip.
private struct VerdictBadge: View {
	let verdict: ToolVerdict
	let sequence: UInt64

	var body: some View {
		Text("Judgess \(verdict.kind)")
			.font(.caption)
			.foregroundStyle(color)
			.padding(.horizontal, 6).padding(.vertical, 1)
			.overlay(Capsule().stroke(color))
			.help("#\(sequence) · \(verdict.detail)")
	}

	private var color: Color {
		switch verdict.kind {
		case "approve": Color(red: 0.29, green: 0.87, blue: 0.5)
		case "deny": Color(red: 0.97, green: 0.44, blue: 0.44)
		case "defer": Color(red: 0.98, green: 0.75, blue: 0.14)
		default: .secondary
		}
	}
}

/// JSON pretty-printed with sorted keys, as the web transcript shows a tool's input; anything else as it is.
func prettyJSON(_ value: String) -> String {
	guard let data = value.data(using: .utf8),
		let json = try? JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed),
		let formatted = try? JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys, .fragmentsAllowed])
	else { return value }
	return String(decoding: formatted, as: UTF8.self)
}
