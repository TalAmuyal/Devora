@testable import AppShell
import CoreBindings
import Layers
import Testing

/// The New Task dialog's selection, filter and keys, as Ember's `NewTaskDialog` and `RepoList` behave.
@MainActor
struct NewTaskDialogTests {
	final class Outcome {
		var created: (title: String, repoPaths: [String])?
		var cancelled = false
		var focusCleared = 0
	}

	let outcome = Outcome()
	let enter = KeyPress(keyCode: KeyCode.returnKey, characters: "\r")
	let down = KeyPress(keyCode: KeyCode.downArrow, characters: "\u{F701}")
	let n = KeyPress(keyCode: 0x2D, characters: "n")

	func dialog(_ names: [String] = ["api", "web", "web-admin"], title: String = "", selected: Set<String> = []) -> NewTaskDialogModel {
		let outcome = outcome
		return NewTaskDialogModel(
			repos: names.map { RepoInfo(name: $0, path: "/repos/\($0)", source: "auto-discovered") },
			title: title, selected: selected, isDuplicating: false,
			clearFocus: { outcome.focusCleared += 1 },
			onCreate: { outcome.created = ($0, $1) }, onCancel: { outcome.cancelled = true })
	}

	@Test func theFilterKeepsTheReposWhoseNameContainsItIgnoringCaseAndTheSelectionSurvivesIt() {
		let model = dialog()
		model.click(model.repos[0])

		model.filter = " WEB "

		#expect(model.visibleRepos.map(\.name) == ["web", "web-admin"])
		#expect(model.activeRepo?.name == "web", "a new filter makes its first match the active one")
		#expect(model.selected == ["/repos/api"])
	}

	@Test func enterInTheFilterTogglesTheActiveRepoAndEnterElsewhereCreatesWithTheSelectionInListOrder() {
		let model = dialog(title: "  Fix login ")
		model.focus = .filter
		#expect(model.handleKey(down))
		#expect(model.handleKey(enter))
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.upArrow, characters: "\u{F700}")))
		#expect(model.handleKey(enter))
		#expect(model.selected == ["/repos/api", "/repos/web"])
		#expect(outcome.created == nil)

		model.focus = .title
		#expect(model.handleKey(enter))

		#expect(outcome.created?.title == "Fix login")
		#expect(outcome.created?.repoPaths == ["/repos/api", "/repos/web"])
	}

	@Test func withoutATitleNothingIsCreated() {
		let model = dialog(title: "   ")

		#expect(model.handleKey(enter))

		#expect(outcome.created == nil)
		#expect(!outcome.cancelled)
	}

	@Test func typingInAFieldIsLeftToTheFieldAndOnAButtonReachesNothingBeneath() {
		let model = dialog()
		#expect(!model.handleKey(n), "the title field types it")
		#expect(!model.handleKey(KeyPress(keyCode: 0x0C, characters: "q")), "q types too, and on a button the stack's dismissal cancels")

		model.focus = .create
		#expect(model.handleKey(n))
		#expect(!model.handleKey(KeyPress(keyCode: 0x0C, characters: "q")))
		#expect(outcome.created == nil && !outcome.cancelled)
	}

	@Test func escapeCancelsWhateverHasTheFocus() {
		let model = dialog()

		#expect(model.handleKey(KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")))

		#expect(outcome.cancelled)
	}

	@Test func tabCyclesTheFieldsAndButtonsInOrderAndAButtonTakesTheFocusFromTheFields() {
		let model = dialog()
		var order: [NewTaskDialogModel.Focus] = [model.focus]
		for _ in 0..<5 {
			model.cycleFocus(backward: false)
			order.append(model.focus)
		}
		#expect(order == [.title, .cloneRepo, .filter, .cancel, .create, .title])
		#expect(outcome.focusCleared == 3)

		model.cycleFocus(backward: true)
		#expect(model.focus == .create)
	}

	@Test func withoutReposTabSkipsTheFilter() {
		let model = dialog([])

		model.cycleFocus(backward: false)
		model.cycleFocus(backward: false)

		#expect(model.focus == .cancel)
	}

	@Test func spacePressesTheFocusedButton() {
		let model = dialog(title: "Task")
		var clonesAsked = 0
		model.cloneRepo = { clonesAsked += 1 }
		model.focus = .cloneRepo
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.space, characters: " ")))
		#expect(clonesAsked == 1)

		model.focus = .cancel
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.space, characters: " ")))

		#expect(outcome.cancelled)
	}

	@Test func aCloneListsTheReposAgainWithTheCloneSelectedKeepingTheTitleAndTheSelection() {
		let model = dialog(["api", "web"], title: "Fix login", selected: ["/repos/api"])

		model.reposChanged(
			["api", "cloned", "web"].map { RepoInfo(name: $0, path: "/repos/\($0)", source: "auto-discovered") },
			selecting: "/repos/cloned")

		#expect(model.repos.map(\.name) == ["api", "cloned", "web"])
		#expect(model.selected == ["/repos/api", "/repos/cloned"])
		#expect(model.title == "Fix login")
	}
}
