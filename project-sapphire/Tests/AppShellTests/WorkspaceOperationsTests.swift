@testable import AppShell
import CoreBindings
import Layers
import Testing

/// Remove Task goes ahead at once only when nothing would be lost, as Ember's hub decides.
struct RemoveTaskDecisionTests {
	private func repo(_ name: String, branch: String = "abc1234", detached: Bool = true, modified: UInt32 = 0, untracked: UInt32 = 0) -> RepoStatus {
		RepoStatus(name: name, branch: branch, isDetached: detached, modified: modified, untracked: untracked)
	}

	@Test func cleanAndDetachedReposNeedNoConfirmation() {
		let status = WorkspaceStatus(repos: [repo("api"), repo("web")], error: nil)
		#expect(RemoveTaskDecision(status: status) == .removeNow)
	}

	@Test func eachRepoThatWouldLoseSomethingIsListedWithWhatAndWhy() {
		let status = WorkspaceStatus(
			repos: [repo("api", untracked: 1), repo("docs"), repo("web", branch: "feature", detached: false, modified: 2)], error: nil)

		#expect(RemoveTaskDecision(status: status) == .confirm(problems: [
			.init(name: "api", detail: "uncommitted changes (will be discarded)"),
			.init(name: "web", detail: "uncommitted changes (will be discarded); on branch 'feature' (will be detached)"),
		]))
	}

	@Test func anUnknownStatusAsksWithNothingListed() {
		#expect(RemoveTaskDecision(status: nil) == .confirm(problems: []), "the status is still loading")
		#expect(RemoveTaskDecision(status: WorkspaceStatus(repos: [], error: "gone")) == .confirm(problems: []))
	}
}

/// The confirmation dialog's keys, as Ember's: Escape cancels, Enter confirms, Tab moves the focus between the two buttons and Space presses the focused one.
@MainActor
struct ConfirmationDialogKeyTests {
	final class Outcome {
		var value: String?
	}

	let outcome = Outcome()
	let model: ConfirmationDialogModel

	init() {
		let outcome = outcome
		model = ConfirmationDialogModel(
			Confirmation(title: "Sure?", intro: "", confirmLabel: "Do It"),
			onConfirm: { outcome.value = "confirmed" }, onCancel: { outcome.value = "cancelled" })
	}

	@Test func escapeCancels() {
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")))
		#expect(outcome.value == "cancelled")
	}

	@Test func enterConfirmsWhicheverButtonHasTheFocus() {
		model.cycleFocus(backward: false)
		#expect(model.focused == .cancel)
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.returnKey, characters: "\r")))
		#expect(outcome.value == "confirmed")
	}

	@Test func spacePressesTheFocusedButton() {
		model.cycleFocus(backward: true)
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.space, characters: " ")))
		#expect(outcome.value == "cancelled")
	}

	@Test func qIsLeftToTheStacksDismissalAndOtherKeysReachNothingBeneath() {
		#expect(!model.handleKey(KeyPress(keyCode: 0x0C, characters: "q")))
		#expect(model.handleKey(KeyPress(keyCode: 0x26, characters: "j")))
		#expect(outcome.value == nil)
	}
}
