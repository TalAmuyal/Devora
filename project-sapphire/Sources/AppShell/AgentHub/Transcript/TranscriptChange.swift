import CoreBindings

/// How a transcript's rows changed from one update to the next, as the table applies it.
enum TranscriptChange: Equatable {
	case none
	/// `count` older rows went in at the top; the rest are as they were.
	case prepended(count: Int)
	/// Rows kept their positions: some changed in place, and rows were added or dropped at either end.
	case updated(changed: [Int], removedFromTop: Int, appended: Int, removedFromBottom: Int)
	/// Anything else: the table loads every row again.
	case reloaded

	init(from old: [TranscriptItem], to new: [TranscriptItem]) {
		guard old != new else {
			self = .none
			return
		}
		guard let oldFirst = old.first, let newFirst = new.first else {
			self = .reloaded
			return
		}
		// Older history: the old rows follow the new ones, unchanged.
		if let start = new.firstIndex(where: { $0.sequence == oldFirst.sequence }), start > 0,
			new.count - start == old.count, new[start...].elementsEqual(old)
		{
			self = .prepended(count: start)
			return
		}
		// Following the bottom trims the oldest rows (the 200-item tail) while new ones stream in at the end.
		guard let shift = old.firstIndex(where: { $0.sequence == newFirst.sequence }) else {
			self = .reloaded
			return
		}
		let kept = old[shift...]
		let shared = min(kept.count, new.count)
		guard zip(kept.prefix(shared), new.prefix(shared)).allSatisfy({ $0.sequence == $1.sequence }) else {
			self = .reloaded
			return
		}
		self = .updated(
			changed: (0..<shared).filter { kept[shift + $0] != new[$0] },
			removedFromTop: shift,
			appended: max(0, new.count - shared),
			removedFromBottom: max(0, kept.count - shared))
	}
}
