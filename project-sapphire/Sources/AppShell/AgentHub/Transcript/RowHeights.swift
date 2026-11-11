import CoreBindings
import Foundation

/// The transcript's row heights: exact for a row measured at its current content, expansion and width, estimated
/// otherwise.
///
/// Measuring a hosted SwiftUI row costs about a millisecond, and `NSTableView` asks for hundreds of heights at once, so
/// the table never measures when it asks: it gets an estimate, and the rows are measured near the visible ones first.
/// A row's estimate is its last measured height when only the width changed, and otherwise a guess from its kind and
/// text length, scaled by how far such guesses were from the measurements so far.
struct RowHeights {
	private struct Entry {
		let row: TranscriptEntry
		let expanded: Bool
		let width: CGFloat
		let height: CGFloat
	}

	private var entries: [TranscriptEntry.Key: Entry] = [:]
	/// Per kind and expansion: the sum of measured heights and of the guesses for those same rows.
	private var calibration: [Shape: (measured: CGFloat, guessed: CGFloat)] = [:]

	func isExact(_ row: TranscriptEntry, expanded: Bool = false, width: CGFloat) -> Bool {
		guard let entry = entries[row.key] else { return false }
		return entry.width == width && entry.row == row && entry.expanded == expanded
	}

	func height(_ row: TranscriptEntry, expanded: Bool = false, width: CGFloat) -> CGFloat {
		if let entry = entries[row.key], entry.expanded == expanded {
			if entry.row == row { return entry.height }
			// Streaming grows a row; its old height is closer than a guess, and the row is measured again soon anyway.
			if Kind(row) == Kind(entry.row) { return max(entry.height, estimate(row, expanded: expanded, width: width)) }
		}
		return estimate(row, expanded: expanded, width: width)
	}

	mutating func record(_ row: TranscriptEntry, expanded: Bool = false, width: CGFloat, height: CGFloat) {
		let shape = Shape(row, expanded: expanded)
		var sums = calibration[shape] ?? (0, 0)
		sums.measured += height
		sums.guessed += Self.guess(row, expanded: expanded, width: width)
		calibration[shape] = sums
		entries[row.key] = Entry(row: row, expanded: expanded, width: width, height: height)
	}

	/// Forgets the rows no longer in the transcript.
	mutating func keep(only keys: Set<TranscriptEntry.Key>) {
		entries = entries.filter { keys.contains($0.key) }
	}

	func estimate(_ row: TranscriptEntry, expanded: Bool = false, width: CGFloat) -> CGFloat {
		let guess = Self.guess(row, expanded: expanded, width: width)
		guard let sums = calibration[Shape(row, expanded: expanded)], sums.guessed > 0 else { return guess }
		return (guess * min(3, max(0.3, sums.measured / sums.guessed))).rounded()
	}

	/// A rough height: a kind's chrome plus its text's lines, at a body font's line height and average glyph width.
	/// A collapsed thinking or tool row is only its summary.
	static func guess(_ row: TranscriptEntry, expanded: Bool = false, width: CGFloat) -> CGFloat {
		let text: String
		var chrome: CGFloat = 24
		switch row {
		case .card(let card):
			// The tool, the reason and the buttons, around the input and the rules.
			text = prettyJSON(card.question.inputJson) + "\n" + (card.rules ?? []).joined(separator: "\n")
			chrome = 110
		case .item(let item):
			switch item.content {
			case .user(let body, let error): text = body + (error ?? "")
			case .assistant(let body): text = body
			case .thinking(let body, _):
				guard expanded else { return 32 }
				text = body
				chrome = 44
			case .tool(_, _, let input, _, let result):
				guard expanded else { return 32 }
				text = input + "\n" + (result?.text ?? "")
				chrome = 48
			case .system(_, let body, _): text = body
			case .unknown(let json): text = json
			}
		}
		let columns = max(10, (width - 120) / 7.5)
		let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
			.reduce(CGFloat(0)) { $0 + max(1, (CGFloat($1.count) / columns).rounded(.up)) }
		return (chrome + lines * 18).rounded()
	}

	enum Kind: Hashable {
		case user, assistant, thinking, tool, system, card

		init(_ row: TranscriptEntry) {
			switch row {
			case .card: self = .card
			case .item(let item): self.init(item)
			}
		}

		init(_ item: TranscriptItem) {
			switch item.content {
			case .user: self = .user
			case .assistant: self = .assistant
			case .thinking: self = .thinking
			case .tool: self = .tool
			case .system, .unknown: self = .system
			}
		}
	}

	/// What a guess is calibrated by: collapsed and expanded rows of one kind differ far more than guesses can say.
	private struct Shape: Hashable {
		let kind: Kind
		let expanded: Bool

		init(_ row: TranscriptEntry, expanded: Bool) {
			kind = Kind(row)
			self.expanded = expanded
		}
	}
}

extension RowHeights {
	func isExact(_ item: TranscriptItem, expanded: Bool = false, width: CGFloat) -> Bool {
		isExact(.item(item), expanded: expanded, width: width)
	}

	func height(_ item: TranscriptItem, expanded: Bool = false, width: CGFloat) -> CGFloat {
		height(.item(item), expanded: expanded, width: width)
	}

	mutating func record(_ item: TranscriptItem, expanded: Bool = false, width: CGFloat, height: CGFloat) {
		record(.item(item), expanded: expanded, width: width, height: height)
	}

	func estimate(_ item: TranscriptItem, expanded: Bool = false, width: CGFloat) -> CGFloat {
		estimate(.item(item), expanded: expanded, width: width)
	}

	static func guess(_ item: TranscriptItem, expanded: Bool = false, width: CGFloat) -> CGFloat {
		guess(.item(item), expanded: expanded, width: width)
	}

	mutating func keep(only sequences: Set<UInt64>) {
		keep(only: Set(sequences.map(TranscriptEntry.Key.item)))
	}
}
