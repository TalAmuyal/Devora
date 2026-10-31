@testable import AppShell
import CoreBindings
import Foundation
import Layers
import Store
import Testing

private func workspace(_ id: String, title: String = "", repos: [String] = [], active: Bool = true) -> Workspace {
	Workspace(id: id, path: "/profile/workspaces/\(id)", profilePath: "/profile", title: title, repos: repos, active: active)
}

private func repo(_ name: String, modified: UInt32 = 0, untracked: UInt32 = 0) -> RepoStatus {
	RepoStatus(name: name, branch: "main", isDetached: false, modified: modified, untracked: untracked)
}

/// The Workspace Hub's list: which workspaces it shows, their dots, and the cursor over them.
struct WorkspaceListTests {
	let workspaces = [
		workspace("ws-1", title: "Fix the Build", repos: ["api"]),
		workspace("ws-2", repos: ["web"], active: false),
		workspace("ws-3", title: "Docs", repos: ["Website"]),
	]

	@Test func categoriesSplitActiveFromInactive() {
		#expect(workspaces.filtered(category: .active, filter: "").map(\.id) == ["ws-1", "ws-3"])
		#expect(workspaces.filtered(category: .inactive, filter: "").map(\.id) == ["ws-2"])
		#expect(workspaces.filtered(category: .all, filter: "").map(\.id) == ["ws-1", "ws-2", "ws-3"])
	}

	@Test func theFilterIgnoresCaseAndMatchesTheTitleTheIdOrARepo() {
		#expect(workspaces.filtered(category: .all, filter: "build").map(\.id) == ["ws-1"])
		#expect(workspaces.filtered(category: .all, filter: "WS-2").map(\.id) == ["ws-2"])
		#expect(workspaces.filtered(category: .all, filter: "web").map(\.id) == ["ws-2", "ws-3"])
		#expect(workspaces.filtered(category: .active, filter: "web").map(\.id) == ["ws-3"])
	}

	@Test func theDotShowsChangesOnceTheStatusIsInAndAnUnreadableStatusAboveAll() {
		let active = workspace("ws-1", title: "Task")
		#expect(active.dot(status: nil) == .pending)
		#expect(active.dot(status: WorkspaceStatus(repos: [repo("a")], error: nil)) == .clean)
		#expect(active.dot(status: WorkspaceStatus(repos: [repo("a"), repo("b", untracked: 1)], error: nil)) == .modified)
		#expect(active.dot(status: WorkspaceStatus(repos: [], error: "gone")) == .error)

		let inactive = workspace("ws-2", active: false)
		#expect(inactive.dot(status: WorkspaceStatus(repos: [repo("a", modified: 2)], error: nil)) == .pending)
		#expect(inactive.dot(status: WorkspaceStatus(repos: [], error: "gone")) == .error)
	}

	@Test func theCursorStopsAtEitherEndAndFollowsAShorterList() {
		var cursor = ListCursor()
		#expect(cursor.position(in: 0) == nil)
		cursor.move(by: -1, in: 3)
		#expect(cursor.position(in: 3) == 0)
		cursor.move(by: 5, in: 3)
		#expect(cursor.position(in: 3) == 2)
		#expect(cursor.position(in: 2) == 1, "a refresh that drops rows keeps the last one selected")
		cursor.filterChanged(count: 2)
		#expect(cursor.position(in: 2) == 0, "a filter that drops the selected row starts over at the top")
	}
}

struct VimScrollTests {
	@Test func motionsAreFractionsOfTheViewport() {
		#expect(VimScroll.delta(for: KeyPress(keyCode: 0x26, characters: "j"), viewportHeight: 500) == 50)
		#expect(VimScroll.delta(for: KeyPress(keyCode: KeyCode.upArrow), viewportHeight: 500) == -50)
		#expect(VimScroll.delta(for: KeyPress(keyCode: KeyCode.d, modifiers: .control, characters: "\u{04}"), viewportHeight: 500) == 250)
		#expect(VimScroll.delta(for: KeyPress(keyCode: KeyCode.u, modifiers: .control, characters: "\u{15}"), viewportHeight: 500) == -250)
		#expect(VimScroll.delta(for: KeyPress(keyCode: 0x26, modifiers: .shift, characters: "J"), viewportHeight: 500) == nil)
	}
}

/// The hub's keys and the filter field: while it is being edited, keys type into it, and Escape leaves it.
@MainActor
struct WorkspaceHubKeyTests {
	final class Focus {
		var editing = false
		var cleared = 0
	}

	let focus = Focus()
	let model: WorkspaceHubModel

	init() throws {
		let log = try AppLog.open(path: FileManager.default.temporaryDirectory.appending(path: "hub-\(UUID()).log").path)
		let store = Store(configPath: "/nonexistent/config.json", state: .inMemory, log: log)
		let focus = focus
		model = WorkspaceHubModel(store: store, isEditingText: { focus.editing }, clearFocus: { focus.cleared += 1 })
	}

	@Test func qAndEscapeAreLeftToTheStacksDismissal() {
		#expect(!model.handleKey(KeyPress(keyCode: 0x0C, characters: "q")))
		#expect(!model.handleKey(KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")))
	}

	@Test func fFocusesTheFilterAndDigitsPickTheCategory() {
		#expect(model.handleKey(KeyPress(keyCode: 0x03, characters: "f")))
		#expect(model.filterFocused)
		#expect(model.handleKey(KeyPress(keyCode: 0x14, characters: "3")))
		#expect(model.category == .all)
	}

	@Test func whileTheFilterIsEditedKeysTypeAndEscapeLeavesIt() {
		focus.editing = true
		model.filterFocused = true
		#expect(!model.handleKey(KeyPress(keyCode: 0x0C, characters: "q")))
		#expect(!model.handleKey(KeyPress(keyCode: 0x14, characters: "3")))
		#expect(model.category == .active)

		#expect(model.handleKey(KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")))
		#expect(!model.filterFocused)
		#expect(focus.cleared == 1)
	}
}
