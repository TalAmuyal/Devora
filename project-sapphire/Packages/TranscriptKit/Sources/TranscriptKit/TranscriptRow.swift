import AppKit
import SwiftUI

/// What one row draws: its item, the item's Markdown (parsed once per content), whether a thinking or tool row is
/// expanded, and its Mermaid diagrams (nil while it streams, when they show as their source).
public struct TranscriptRowModel: Equatable {
	let item: TranscriptItem
	let document: MarkdownDocument?
	let expanded: Bool
	let diagrams: [String: DiagramOutcome]?

	init(_ item: TranscriptItem, document: MarkdownDocument?, expanded: Bool = false, diagrams: [String: DiagramOutcome]? = nil) {
		self.item = item
		self.document = document
		self.expanded = expanded
		self.diagrams = diagrams
	}

	public enum Kind: String {
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
		case .user(let text, _), .assistant(let text, _), .thinking(let text, _): text
		case .tool, .system, .unknown: nil
		}
	}

	/// The text a row has (and the test bridge reads), collapsed or not, as the web transcript's row text.
	var plainText: String {
		switch item.content {
		case .user(let text, let error): text + (error.map { "\n\n\($0)" } ?? "")
		case .assistant(let text, _): text
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
		case .user(let text, _), .assistant(let text, _), .thinking(let text, _): text
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

	/// The row's hover title: its sequence, and for a reply the model that wrote it.
	var helpTitle: String {
		guard case .assistant(_, let model?) = item.content else { return "#\(item.sequence)" }
		return "#\(item.sequence) · \(model)"
	}

	/// What VoiceOver reads after the kind: the text, without what a collapsed row hides.
	var accessibilityValue: String {
		switch item.content {
		case .system(.modelChanged, _, let detailJson):
			// The host words the change for speech ("from … to …"), since an arrow reads poorly.
			(try? JSONSerialization.jsonObject(with: Data(detailJson.utf8)) as? [String: Any])?["spoken"] as? String
				?? plainText
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
	@Environment(\.transcriptTheme) private var theme
	@Environment(\.colorScheme) private var scheme

	static let margin: CGFloat = 16
	/// The room a reply leaves on its right, so that the user's bubbles stand out from it.
	static let replyTrailing: CGFloat = 48
	/// How much narrower than its row a reply is, which is the width its diagrams are drawn at.
	static let replyInset = 2 * margin + replyTrailing

	var body: some View {
		placed
			.padding(.horizontal, Self.margin).padding(.vertical, 4)
			.textSelection(.enabled)
			.help(model.helpTitle)
			.environment(\.openURL, OpenURLAction { Self.opensLink($0) ? .systemAction : .discarded })
			.environment(\.diagrams, model.diagrams)
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

	/// The row in the conversation: the user's message in a bubble on the right, the agent's reply as text on the page,
	/// and the rest (the agent's working, the session's notes, errors) in a grey bubble on the left.
	@ViewBuilder private var placed: some View {
		switch model.item.content {
		case .user(let text, let error):
			VStack(alignment: .trailing, spacing: 4) {
				UserBubbleLayout {
					markdown(text)
						.foregroundStyle(.white)
						.environment(\.transcriptTheme, .onBubble)
						.padding(.horizontal, 12).padding(.vertical, 8)
						.background(Color(nsColor: .systemBlue), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
				}
				if let error { Text(error).font(.callout).foregroundStyle(theme.error) }
			}
			.frame(maxWidth: .infinity, alignment: .trailing)
		case .assistant(let text, _):
			VStack(alignment: .leading, spacing: 4) {
				markdown(text)
				stateNote
			}
			.frame(maxWidth: .infinity, alignment: .leading)
			.padding(.trailing, Self.replyTrailing)
		default:
			VStack(alignment: .leading, spacing: 4) {
				content
				stateNote
			}
			.padding(.horizontal, 10).padding(.vertical, 5)
			.background(bubbleFill, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
			.frame(maxWidth: .infinity, alignment: .leading)
			.padding(.trailing, Self.replyTrailing)
		}
	}

	/// A grey bubble's fill: tinted red for an error, which shows whatever the system messages' toggle says.
	private var bubbleFill: Color {
		model.item.isError ? Color(nsColor: .systemRed).opacity(scheme == .dark ? 0.22 : 0.12) : Color(nsColor: .quaternarySystemFill)
	}

	/// What became of an item that didn't finish, under it.
	@ViewBuilder private var stateNote: some View {
		switch model.item.state {
		case .interrupted: Label("Interrupted", systemImage: "stop.circle").font(.caption).foregroundStyle(theme.secondary)
		case .failed: Label("Failed", systemImage: "exclamationmark.triangle").font(.caption).foregroundStyle(theme.error)
		default: EmptyView()
		}
	}

	/// A grey bubble's content.
	@ViewBuilder private var content: some View {
		switch model.item.content {
		case .user, .assistant:
			EmptyView()
		case .thinking(let text, _):
			VStack(alignment: .leading, spacing: 4) {
				summary { Text("Thinking") }
				if model.expanded { markdown(text).foregroundStyle(theme.secondary) }
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
			Text(model.plainText).font(.callout).foregroundStyle(model.item.isError ? theme.text : theme.secondary)
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
			.foregroundStyle(theme.secondary)
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

/// The user's bubble: as wide as its text, up to 70% of the row, at the row's trailing edge. A layout rather than a
/// frame, so that the row's measured height is the bubble's at that width.
private struct UserBubbleLayout: Layout {
	static let widthFraction: CGFloat = 0.7

	func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
		guard let bubble = subviews.first else { return .zero }
		let size = bubble.sizeThatFits(bubbleProposal(proposal.width))
		return CGSize(width: proposal.width ?? size.width, height: size.height)
	}

	func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
		guard let bubble = subviews.first else { return }
		let size = bubble.sizeThatFits(bubbleProposal(bounds.width))
		bubble.place(at: CGPoint(x: bounds.maxX, y: bounds.minY), anchor: .topTrailing, proposal: ProposedViewSize(size))
	}

	private func bubbleProposal(_ width: CGFloat?) -> ProposedViewSize {
		ProposedViewSize(width: width.map { $0 * Self.widthFraction }, height: nil)
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
