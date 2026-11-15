import CoreBindings

/// One row of the native transcript: an item, or a permission card the host builds from a question.
enum TranscriptEntry: Equatable {
	case item(TranscriptItem)
	case card(PermissionCard)

	/// What identifies a row across updates: its item's sequence, or its question's request id.
	enum Key: Hashable {
		case item(UInt64)
		case card(String)
	}

	var key: Key {
		switch self {
		case .item(let item): .item(item.sequence)
		case .card(let card): .card(card.question.requestId)
		}
	}

	var item: TranscriptItem? {
		if case .item(let item) = self { item } else { nil }
	}

	/// The rows for `items` and `questions`: each question's card right after its tool's row, as the web transcript
	/// places it, or at the end when that row isn't in the transcript (not arrived yet, or before the loaded page).
	static func layout(_ items: [TranscriptItem], questions: [PermissionQuestion], generation: UInt64) -> [TranscriptEntry] {
		guard !questions.isEmpty else { return items.map(TranscriptEntry.item) }
		var byTool: [String: [PermissionQuestion]] = [:]
		for question in questions { byTool[question.toolUseId, default: []].append(question) }
		var placed = Set<String>()
		var entries: [TranscriptEntry] = []
		entries.reserveCapacity(items.count + questions.count)
		for item in items {
			entries.append(.item(item))
			if case .tool(let id, _, _, _, _) = item.content, let owned = byTool[id], !placed.contains(id) {
				placed.insert(id)
				entries += owned.map { .card(PermissionCard(question: $0, generation: generation)) }
			}
		}
		entries += questions.filter { !placed.contains($0.toolUseId) }.map { .card(PermissionCard(question: $0, generation: generation)) }
		return entries
	}
}

/// How a transcript's rows changed from one update to the next, as the table applies it.
enum TranscriptChange: Equatable {
	case none
	/// `count` older rows went in at the top; the rest are as they were.
	case prepended(count: Int)
	/// Rows kept their positions: some changed in place, and rows were added or dropped at either end.
	case updated(changed: [Int], removedFromTop: Int, appended: Int, removedFromBottom: Int)
	/// Anything else (a card coming or going between rows, too): the table loads every row again.
	case reloaded

	init(from old: [TranscriptItem], to new: [TranscriptItem]) {
		self.init(from: old.map(TranscriptEntry.item), to: new.map(TranscriptEntry.item))
	}

	init(from old: [TranscriptEntry], to new: [TranscriptEntry]) {
		guard old != new else {
			self = .none
			return
		}
		guard let oldFirst = old.first, let newFirst = new.first else {
			self = .reloaded
			return
		}
		// Older history: the old rows follow the new ones, unchanged.
		if let start = new.firstIndex(where: { $0.key == oldFirst.key }), start > 0,
			new.count - start == old.count, new[start...].elementsEqual(old)
		{
			self = .prepended(count: start)
			return
		}
		// Following the bottom trims the oldest rows (the 200-item tail) while new ones stream in at the end.
		guard let shift = old.firstIndex(where: { $0.key == newFirst.key }) else {
			self = .reloaded
			return
		}
		let kept = old[shift...]
		let shared = min(kept.count, new.count)
		guard zip(kept.prefix(shared), new.prefix(shared)).allSatisfy({ $0.key == $1.key }) else {
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
