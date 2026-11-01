@testable import AppShell
import CoreBindings
import Foundation
import Layers
import Store
import Testing

/// The Add Repo and Clone Repo dialogs' forms, keys and progress phase, as Ember's `AddRepoDialog` and `CloneRepoDialog` behave, over a store on the real core.
@MainActor
struct RepoDialogTests {
	final class Calls {
		var closed = 0
		var added = 0
		var cloned: [CreatedWorkspace] = []
		var focusCleared = 0
	}

	let calls = Calls()
	let store: Store
	let enter = KeyPress(keyCode: KeyCode.returnKey, characters: "\r")
	let escape = KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")
	let space = KeyPress(keyCode: KeyCode.space, characters: " ")
	let down = KeyPress(keyCode: KeyCode.downArrow, characters: "\u{F701}")
	let q = KeyPress(keyCode: 0x0C, characters: "q")

	init() throws {
		let log = try AppLog.open(path: FileManager.default.temporaryDirectory.appending(path: "repo-dialogs-\(UUID()).log").path)
		store = Store(configPath: "/nonexistent/config.json", state: .inMemory, log: log)
	}

	/// The workspace doesn't exist, so an add the dialog starts fails.
	func addRepo(_ names: [String] = ["api", "web", "web-admin"]) -> AddRepoDialogModel {
		let calls = calls
		return AddRepoDialogModel(
			target: AddRepoTarget(workspacePath: "/nonexistent/workspaces/ws-1", profilePath: "/nonexistent"),
			repos: names.map { RepoInfo(name: $0, path: "/nonexistent/repos/\($0)", source: "auto-discovered") },
			store: store, clearFocus: { calls.focusCleared += 1 }, close: { calls.closed += 1 }, onAdded: { calls.added += 1 })
	}

	func cloneRepo() -> CloneRepoDialogModel {
		let calls = calls
		return CloneRepoDialogModel(
			profilePath: "/profiles/work", store: store, clearFocus: { calls.focusCleared += 1 }, close: { calls.closed += 1 },
			onCloned: { calls.cloned.append($0) })
	}

	@Test func theWorktreeIsNamedAfterTheRepoWithTheTrimmedPostfix() {
		#expect(AddRepoTarget.worktreeDirName(repo: "api", postfix: "") == "api")
		#expect(AddRepoTarget.worktreeDirName(repo: "api", postfix: "   ") == "api")
		#expect(AddRepoTarget.worktreeDirName(repo: "api", postfix: " ref ") == "api-ref")
	}

	@Test func theFirstRepoIsSelectedAndTheFilterAndTheArrowsMoveTheSelection() {
		let model = addRepo()
		#expect(model.selectedRepo?.name == "api", "Add Repo is actionable at once")

		model.filter = " WEB "
		#expect(model.visibleRepos.map(\.name) == ["web", "web-admin"])
		#expect(model.selectedRepo?.name == "web", "a new filter selects its first match")
		#expect(model.handleKey(down))
		#expect(model.selectedRepo?.name == "web-admin")

		model.filter = "nothing"
		#expect(model.selectedRepo == nil)
		#expect(model.handleKey(enter))
		#expect(model.operation == nil, "with no repo selected, Enter adds nothing")
	}

	@Test func leavingTheFilterKeepsTheSelection() {
		let model = addRepo()
		model.click(model.repos[1])

		// What SwiftUI does as the focus leaves the filter field.
		model.filter = model.filter

		#expect(model.selectedRepo?.name == "web")
	}

	@Test func enterAddsTheSelectedRepoAndAFailureStaysUpUntilClosed() async throws {
		let model = addRepo()
		model.filter = "web"
		model.postfix = " ref "
		model.focus = .postfix

		#expect(model.handleKey(enter))

		let operation = try #require(model.operation)
		#expect(operation.title == "Adding: web-ref")
		#expect(model.focus == .cancel, "the fields give the keyboard up")
		try await eventually { model.operation?.failure != nil }
		#expect(model.operation?.action == "Close")
		#expect(calls.closed == 0, "a failure leaves the dialog up")
		try await eventually { !store.errorBanners.visible.isEmpty }
		#expect(store.scrapeErrors().count == 1)

		#expect(model.handleKey(escape))
		#expect(calls.closed == 1)
		#expect(calls.added == 0)
	}

	@Test func escapeCancelsTheFormAndQIsLeftToTheStacksDismissalWhichCancelsItOnAButton() {
		let model = addRepo()
		#expect(!model.handleKey(q), "the filter types it")
		#expect(model.handleKey(escape))
		#expect(calls.closed == 1)

		model.focus = .add
		#expect(!model.handleKey(q))
		model.dismiss()
		#expect(calls.closed == 2)
		#expect(model.operation == nil)
	}

	@Test func tabCyclesTheAddRepoFieldsAndButtonsAndSpacePressesTheFocusedOne() {
		let model = addRepo()
		var order: [AddRepoDialogModel.Focus] = [model.focus]
		for _ in 0..<4 {
			model.cycleFocus(backward: false)
			order.append(model.focus)
		}
		#expect(order == [.filter, .postfix, .cancel, .add, .filter])
		model.cycleFocus(backward: true)
		#expect(model.focus == .add)

		model.focus = .cancel
		#expect(model.handleKey(space))
		#expect(calls.closed == 1)
		#expect(model.operation == nil)
	}

	@Test func theCloneHintIsTheCoresDestinationForWhatItCanClone() {
		let model = cloneRepo()
		#expect(model.hint == nil)

		model.url = "https://github.com/org/api/pulls"
		#expect(model.hint == "Clones to:  /profiles/work/repos/api")

		model.url = "not a url"
		#expect(model.hint == nil)
	}

	@Test func enterClonesTheTrimmedURLOnlyWhenThereIsOne() async throws {
		let model = cloneRepo()
		model.url = "   "
		#expect(model.handleKey(enter))
		#expect(model.operation == nil)

		model.url = "  file:///nonexistent/other.git "
		#expect(model.handleKey(enter))

		#expect(model.operation?.title == "Cloning repo")
		try await eventually { model.operation?.failure != nil }
		#expect(store.scrapeErrors().count == 1)
		model.dismiss()
		#expect(calls.closed == 1)
		#expect(calls.cloned.isEmpty)
	}

	@Test func tabCyclesTheCloneFieldAndButtonsAndTypingStaysInTheField() {
		let model = cloneRepo()
		#expect(!model.handleKey(q))
		#expect(!model.handleKey(space))
		var order: [CloneRepoDialogModel.Focus] = [model.focus]
		for _ in 0..<3 {
			model.cycleFocus(backward: false)
			order.append(model.focus)
		}
		#expect(order == [.url, .cancel, .clone, .url])

		model.focus = .cancel
		#expect(model.handleKey(KeyPress(keyCode: 0x2D, characters: "n")), "on a button, nothing beneath sees a key")
		#expect(model.handleKey(escape))
		#expect(calls.closed == 1)
	}
}
