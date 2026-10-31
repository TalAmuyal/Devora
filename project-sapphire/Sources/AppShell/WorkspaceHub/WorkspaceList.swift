import CoreBindings

/// Which workspaces the hub lists: `1`, `2` and `3` pick one.
enum HubCategory: CaseIterable {
	case active
	case inactive
	case all

	var label: String {
		switch self {
		case .active: "Active"
		case .inactive: "Inactive"
		case .all: "All"
		}
	}
}

/// A workspace row's status dot, as Ember's `StatusDot` variants.
enum StatusDot: String {
	case clean
	case modified
	case pending
	case error
}

extension Workspace {
	/// An inactive workspace has no task title; so does an active one whose `task.json` is corrupt, which Ember shows the same way.
	var hasTask: Bool { !title.isEmpty }

	func matches(category: HubCategory) -> Bool {
		switch category {
		case .active: active
		case .inactive: !active
		case .all: true
		}
	}

	/// A case-insensitive substring of the title, the id or a repo's name.
	func matches(filter: String) -> Bool {
		guard !filter.isEmpty else { return true }
		return [title, id].contains { $0.localizedCaseInsensitiveContains(filter) }
			|| repos.contains { $0.localizedCaseInsensitiveContains(filter) }
	}

	/// An unreadable status wins; a workspace without a task, or whose status is still loading, is pending.
	func dot(status: WorkspaceStatus?) -> StatusDot {
		if status?.error != nil { return .error }
		guard hasTask, let status else { return .pending }
		return status.hasChanges ? .modified : .clean
	}
}

extension [Workspace] {
	func filtered(category: HubCategory, filter: String) -> [Workspace] {
		self.filter { $0.matches(category: category) && $0.matches(filter: filter) }
	}
}

extension WorkspaceStatus {
	var modified: UInt32 { repos.reduce(0) { $0 + $1.modified } }
	var untracked: UInt32 { repos.reduce(0) { $0 + $1.untracked } }
	var hasChanges: Bool { modified + untracked > 0 }
}

/// The selected row of a list whose items can change under it (ADR-004's list cursor): the index is kept as is and fitted to the list when read, so a shorter list selects its last row.
struct ListCursor: Equatable {
	private(set) var index = 0

	/// The selected row of a list of `count` rows; nil when it is empty.
	func position(in count: Int) -> Int? {
		count == 0 ? nil : min(max(index, 0), count - 1)
	}

	/// Moves by `delta` rows, stopping at either end.
	mutating func move(by delta: Int, in count: Int) {
		guard let position = position(in: count) else { return }
		index = min(max(position + delta, 0), count - 1)
	}

	mutating func select(_ index: Int) {
		self.index = index
	}

	/// After the filter changed: a selection past the end goes back to the first row, as Ember's does.
	mutating func filterChanged(count: Int) {
		if index >= count { index = 0 }
	}
}
