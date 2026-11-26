import CoreBindings

/// The compaction threshold's range, the core's (`compact_threshold_range`), and what the controls that set it say about
/// a value outside it: the Agent card's field and the Context popover's slider.
enum CompactThreshold {
	static let range: ClosedRange<UInt8> = {
		// A constant of the core's, so it can't fail; there's no range to fall back to that isn't a copy of it.
		guard let range = try? compactThresholdRange() else { preconditionFailure("the core has no compaction threshold range") }
		return range.min...range.max
	}()

	static var placeholder: String { "\(range.lowerBound)–\(range.upperBound)" }

	/// A threshold stored before the range (1 to 100 was valid then), which the core uses as its nearest limit.
	static func limitedNote(stored: UInt8, used: UInt8) -> String {
		"Set to \(stored)%, used as \(used)%: the threshold goes from \(range.lowerBound)% to \(range.upperBound)%"
	}

	/// Under the field while it holds a value the core would refuse.
	static var outsideNote: String { "From \(range.lowerBound)% to \(range.upperBound)%" }

	/// The percentage `text` asks for, if the core would take it.
	static func percent(_ text: String) -> UInt8? {
		UInt8(text.trimmingCharacters(in: .whitespacesAndNewlines)).flatMap { range.contains($0) ? $0 : nil }
	}
}
