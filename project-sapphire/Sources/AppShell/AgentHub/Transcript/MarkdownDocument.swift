import Foundation
import Markdown

/// A transcript item's Markdown as plain values, parsed once by `swift-markdown` and drawn by `MarkdownView`.
/// Syntax it doesn't model (HTML, images, footnotes) keeps its source text, so nothing an agent writes disappears.
struct MarkdownDocument: Equatable {
	let blocks: [MarkdownBlock]
	/// Every Mermaid code block's source, in reading order: found once, since the transcript asks for them on every
	/// height check.
	let diagrams: [String]

	init(_ text: String) {
		self.init(blocks: Self.blocks(Document(parsing: text).children))
	}

	init(blocks: [MarkdownBlock]) {
		self.blocks = blocks
		diagrams = Self.diagrams(in: blocks)
	}

	private static func diagrams(in blocks: [MarkdownBlock]) -> [String] {
		func walk(_ blocks: [MarkdownBlock]) -> [String] {
			blocks.flatMap { block -> [String] in
				switch block {
				case .code(let language, let text): language?.lowercased() == "mermaid" ? [text] : []
				case .list(_, let items): items.flatMap(walk)
				case .quote(let inner): walk(inner)
				case .heading, .paragraph, .table, .thematicBreak: []
				}
			}
		}
		return walk(blocks)
	}

	/// Every link, with its text, in reading order.
	var links: [(text: String, url: URL)] {
		var found: [(text: String, url: URL)] = []
		func runs(_ runs: [InlineRun]) {
			for run in runs { if let link = run.link { found.append((run.text, link)) } }
		}
		func walk(_ blocks: [MarkdownBlock]) {
			for block in blocks {
				switch block {
				case .heading(_, let inline), .paragraph(let inline): runs(inline)
				case .list(_, let items): items.forEach(walk)
				case .table(_, let header, let rows):
					header.forEach(runs)
					rows.forEach { $0.forEach(runs) }
				case .quote(let inner): walk(inner)
				case .code, .thematicBreak: break
				}
			}
		}
		walk(blocks)
		return found
	}

	private static func blocks(_ children: MarkupChildren) -> [MarkdownBlock] {
		children.map { block($0) }
	}

	private static func block(_ markup: any Markup) -> MarkdownBlock {
		switch markup {
		case let heading as Heading:
			return .heading(level: heading.level, inlines(heading))
		case let paragraph as Paragraph:
			return .paragraph(inlines(paragraph))
		case let list as UnorderedList:
			return .list(start: nil, items: list.listItems.map { blocks($0.children) })
		case let list as OrderedList:
			return .list(start: Int(list.startIndex), items: list.listItems.map { blocks($0.children) })
		case let code as CodeBlock:
			let text = code.code.hasSuffix("\n") ? String(code.code.dropLast()) : code.code
			return .code(language: code.language.flatMap { $0.isEmpty ? nil : $0 }, text)
		case let table as Markdown.Table:
			return .table(
				alignments: table.columnAlignments.map(TableAlignment.init),
				header: table.head.cells.map { inlines($0) },
				rows: table.body.rows.map { row in row.cells.map { inlines($0) } })
		case let quote as BlockQuote:
			return .quote(blocks(quote.children))
		case is ThematicBreak:
			return .thematicBreak
		default:
			return .paragraph([InlineRun(markup.format().trimmingCharacters(in: .newlines))])
		}
	}

	private static func inlines(_ markup: any Markup) -> [InlineRun] {
		var runs: [InlineRun] = []
		for child in markup.children { append(child, style: [], link: nil, to: &runs) }
		return runs
	}

	private static func append(_ markup: any Markup, style: InlineStyle, link: URL?, to runs: inout [InlineRun]) {
		func add(_ text: String, _ style: InlineStyle) {
			// Neighbors with the same style and link are one run, so a document compares and draws by its spans.
			if let last = runs.last, last.style == style, last.link == link {
				runs[runs.count - 1] = InlineRun(last.text + text, style: style, link: link)
			} else {
				runs.append(InlineRun(text, style: style, link: link))
			}
		}
		func children(style: InlineStyle, link: URL?) {
			for child in markup.children { append(child, style: style, link: link, to: &runs) }
		}
		switch markup {
		case let text as Markdown.Text: add(text.string, style)
		case let code as InlineCode: add(code.code, style.union(.code))
		case is Emphasis: children(style: style.union(.emphasis), link: link)
		case is Strong: children(style: style.union(.strong), link: link)
		case is Strikethrough: children(style: style.union(.strikethrough), link: link)
		case let anchor as Markdown.Link: children(style: style, link: anchor.destination.flatMap(URL.init(string:)) ?? link)
		case is SoftBreak: add(" ", style)
		case is LineBreak: add("\n", style)
		case let image as Markdown.Image: add(image.plainText, style)
		case let html as InlineHTML: add(html.rawHTML, style)
		default: children(style: style, link: link)
		}
	}
}

indirect enum MarkdownBlock: Equatable {
	case heading(level: Int, [InlineRun])
	case paragraph([InlineRun])
	/// An ordered list has its first number; an unordered one has none.
	case list(start: Int?, items: [[MarkdownBlock]])
	case code(language: String?, String)
	case table(alignments: [TableAlignment], header: [[InlineRun]], rows: [[[InlineRun]]])
	case quote([MarkdownBlock])
	case thematicBreak
}

enum TableAlignment: Equatable {
	case leading, center, trailing

	init(_ alignment: Markdown.Table.ColumnAlignment?) {
		switch alignment {
		case .center: self = .center
		case .right: self = .trailing
		default: self = .leading
		}
	}
}

struct InlineStyle: OptionSet, Hashable {
	let rawValue: UInt8
	static let code = InlineStyle(rawValue: 1)
	static let emphasis = InlineStyle(rawValue: 2)
	static let strong = InlineStyle(rawValue: 4)
	static let strikethrough = InlineStyle(rawValue: 8)
}

struct InlineRun: Equatable {
	let text: String
	let style: InlineStyle
	let link: URL?

	init(_ text: String, style: InlineStyle = [], link: URL? = nil) {
		self.text = text
		self.style = style
		self.link = link
	}
}
