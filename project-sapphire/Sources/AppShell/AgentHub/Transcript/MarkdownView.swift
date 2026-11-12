import SwiftUI

/// Draws a `MarkdownDocument`'s blocks, one SwiftUI view per block.
struct MarkdownView: View {
	let blocks: [MarkdownBlock]

	var body: some View {
		VStack(alignment: .leading, spacing: 6) {
			ForEach(blocks.indices, id: \.self) { index in
				MarkdownBlockView(block: blocks[index])
			}
		}
	}
}

private struct MarkdownBlockView: View {
	let block: MarkdownBlock

	var body: some View {
		switch block {
		case .heading(let level, let runs):
			Text(attributed(runs)).font(level <= 1 ? .title2 : level == 2 ? .title3 : .headline)
		case .paragraph(let runs):
			Text(attributed(runs))
		case .list(let start, let items):
			VStack(alignment: .leading, spacing: 3) {
				ForEach(items.indices, id: \.self) { index in
					HStack(alignment: .firstTextBaseline, spacing: 6) {
						Text(start.map { "\($0 + index)." } ?? "•").monospacedDigit()
						MarkdownView(blocks: items[index])
					}
				}
			}
		case .code(let language, let text) where language?.lowercased() == "mermaid":
			DiagramBlockView(source: text)
		case .code(_, let text):
			CodeBlockView(text: text)
		case .table(let alignments, let header, let rows):
			Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 4) {
				GridRow {
					ForEach(header.indices, id: \.self) { column in
						Text(attributed(header[column])).bold()
							.gridColumnAlignment(horizontal(column < alignments.count ? alignments[column] : .leading))
					}
				}
				Divider()
				ForEach(rows.indices, id: \.self) { row in
					GridRow {
						ForEach(rows[row].indices, id: \.self) { column in
							Text(attributed(rows[row][column]))
						}
					}
				}
			}
		case .quote(let blocks):
			HStack(alignment: .top, spacing: 8) {
				RoundedRectangle(cornerRadius: 1).fill(.secondary).frame(width: 3)
				MarkdownView(blocks: blocks).foregroundStyle(.secondary)
			}
		case .thematicBreak:
			Divider()
		}
	}

	private func horizontal(_ alignment: TableAlignment) -> HorizontalAlignment {
		switch alignment {
		case .leading: .leading
		case .center: .center
		case .trailing: .trailing
		}
	}
}

private struct CodeBlockView: View {
	let text: String

	var body: some View {
		// Sideways, never wrapped: the transcript takes the vertical scrolls over it (`WheelRouting`).
		ScrollView(.horizontal) {
			Text(text).font(.system(.callout, design: .monospaced)).fixedSize(horizontal: true, vertical: false).padding(8)
		}
		.background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 6))
	}
}

/// A block's inline runs as one attributed string.
func attributed(_ runs: [InlineRun]) -> AttributedString {
	var result = AttributedString()
	for run in runs {
		var part = AttributedString(run.text)
		var intent: InlinePresentationIntent = []
		if run.style.contains(.emphasis) { intent.insert(.emphasized) }
		if run.style.contains(.strong) { intent.insert(.stronglyEmphasized) }
		if run.style.contains(.code) {
			part.font = .system(.body, design: .monospaced)
			part.backgroundColor = .secondary.opacity(0.15)
		}
		if !intent.isEmpty { part.inlinePresentationIntent = intent }
		if run.style.contains(.strikethrough) { part.strikethroughStyle = .single }
		if let link = run.link { part.link = link }
		result += part
	}
	return result
}

extension EnvironmentValues {
	/// The row's Mermaid diagrams by source, as the transcript resolved them; nil shows every diagram as its source (a
	/// streaming reply, as the web transcript does, or Markdown outside the transcript).
	@Entry var diagrams: [String: DiagramOutcome]? = nil
}

/// A Mermaid block: its drawn image, a placeholder of a stable height until it is drawn, or its source with the web
/// transcript's warning when it can't be.
private struct DiagramBlockView: View {
	let source: String
	@Environment(\.diagrams) private var diagrams

	static let placeholderHeight: CGFloat = 120

	var body: some View {
		switch diagrams.map({ $0[source] ?? .pending }) {
		case nil:
			CodeBlockView(text: source)
		case .drawn(let drawn):
			Image(nsImage: drawn.image)
				.resizable()
				.aspectRatio(contentMode: .fit)
				.frame(maxWidth: drawn.image.size.width, alignment: .leading)
		case .pending:
			RoundedRectangle(cornerRadius: 6)
				.fill(.quaternary.opacity(0.5))
				.frame(height: Self.placeholderHeight)
				.overlay { ProgressView().controlSize(.small) }
		case .failed:
			VStack(alignment: .leading, spacing: 4) {
				Text("⚠ Could not render this Mermaid diagram — showing source").font(.caption).foregroundStyle(.secondary)
				CodeBlockView(text: source)
			}
		}
	}
}
