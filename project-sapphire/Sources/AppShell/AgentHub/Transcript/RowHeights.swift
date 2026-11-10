import CoreBindings
import Foundation

/// The transcript's row heights: exact for a row measured at its current content and width, estimated otherwise.
///
/// Measuring a hosted SwiftUI row costs about a millisecond, and `NSTableView` asks for hundreds of heights at once, so
/// the table never measures when it asks: it gets an estimate, and the rows are measured near the visible ones first.
/// A row's estimate is its last measured height when only the width changed, and otherwise a guess from its kind and
/// text length, scaled by how far such guesses were from the measurements so far.
struct RowHeights {
	private struct Entry {
		let item: TranscriptItem
		let width: CGFloat
		let height: CGFloat
	}

	private var entries: [UInt64: Entry] = [:]
	/// Per kind: the sum of measured heights and of the guesses for those same rows.
	private var calibration: [Kind: (measured: CGFloat, guessed: CGFloat)] = [:]

	func isExact(_ item: TranscriptItem, width: CGFloat) -> Bool {
		guard let entry = entries[item.sequence] else { return false }
		return entry.width == width && entry.item == item
	}

	func height(_ item: TranscriptItem, width: CGFloat) -> CGFloat {
		if let entry = entries[item.sequence] {
			if entry.item == item { return entry.height }
			// Streaming grows a row; its old height is closer than a guess, and the row is measured again soon anyway.
			if Kind(item) == Kind(entry.item) { return max(entry.height, estimate(item, width: width)) }
		}
		return estimate(item, width: width)
	}

	mutating func record(_ item: TranscriptItem, width: CGFloat, height: CGFloat) {
		let kind = Kind(item)
		var sums = calibration[kind] ?? (0, 0)
		sums.measured += height
		sums.guessed += Self.guess(item, width: width)
		calibration[kind] = sums
		entries[item.sequence] = Entry(item: item, width: width, height: height)
	}

	/// Forgets the rows no longer in the transcript.
	mutating func keep(only sequences: Set<UInt64>) {
		entries = entries.filter { sequences.contains($0.key) }
	}

	func estimate(_ item: TranscriptItem, width: CGFloat) -> CGFloat {
		let guess = Self.guess(item, width: width)
		guard let sums = calibration[Kind(item)], sums.guessed > 0 else { return guess }
		return (guess * min(3, max(0.3, sums.measured / sums.guessed))).rounded()
	}

	/// A rough height: a kind's chrome plus its text's lines, at a body font's line height and average glyph width.
	static func guess(_ item: TranscriptItem, width: CGFloat) -> CGFloat {
		let text: String
		var chrome: CGFloat = 24
		switch item.content {
		case .user(let body, let error): text = body + (error ?? "")
		case .assistant(let body): text = body
		case .thinking(let body, _): text = body
		case .tool(_, _, _, _, let result):
			text = result?.text ?? ""
			chrome = 48
		case .system(_, let body, _): text = body
		case .unknown(let json): text = json
		}
		let columns = max(10, (width - 120) / 7.5)
		let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
			.reduce(CGFloat(0)) { $0 + max(1, (CGFloat($1.count) / columns).rounded(.up)) }
		return (chrome + lines * 18).rounded()
	}

	enum Kind: Hashable {
		case user, assistant, thinking, tool, system

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
}
