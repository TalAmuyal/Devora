import CoreBindings
import Layers
import Store
import SwiftUI

/// One open Add Repo dialog, Ember's `AddRepoDialog`: a filterable single-select list of the profile's repos and an optional name postfix, then the add's progress in its place (`RepoOperationPhase`), as a modal layer over the window (`Surfaces.openAddRepoDialog`).
///
/// Its keys are Ember's: Escape cancels and Enter adds the selected repo, whatever has the focus, ↑ and ↓ move the selection while the filter has the focus, and `q` goes on to the stack's dismissal, which types in a field and cancels on a button.
/// Tab and Shift+Tab move the focus around its fields and buttons without ever leaving it, and Space presses the focused button, whose focus it draws itself.
@MainActor
@Observable
final class AddRepoDialogModel {
	static let layerName = "add-repo-dialog"

	/// In Tab order, which is the order they are drawn in.
	enum Focus: CaseIterable {
		case filter
		case postfix
		case cancel
		case add

		var isTextField: Bool { self == .filter || self == .postfix }
	}

	let target: AddRepoTarget
	/// The repos of the workspace's profile, in its order.
	let repos: [RepoInfo]
	/// A new filter moves the cursor to its first match; SwiftUI writes the field's same text back as it leaves the field, which moves nothing.
	var filter = "" {
		didSet { if filter != oldValue { cursor.select(0) } }
	}
	var postfix = ""
	var focus = Focus.filter {
		didSet { if !focus.isTextField { clearFocus() } }
	}
	/// The selection: the active row, as Ember's single-select list has it.
	private(set) var cursor = ListCursor()
	/// Once the add has started, the dialog shows its progress instead of the form.
	private(set) var operation: RepoOperationPhase?

	@ObservationIgnored private let store: Store
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private let close: () -> Void
	@ObservationIgnored private let onAdded: () -> Void

	/// `close` closes the dialog; `onAdded` runs once a repo was added, after the store has listed the workspaces again.
	init(
		target: AddRepoTarget, repos: [RepoInfo], store: Store,
		clearFocus: @escaping () -> Void, close: @escaping () -> Void, onAdded: @escaping () -> Void
	) {
		self.target = target
		self.repos = repos
		self.store = store
		self.clearFocus = clearFocus
		self.close = close
		self.onAdded = onAdded
	}

	var visibleRepos: [RepoInfo] { repos.matching(filter) }

	/// None while the filter matches no repo.
	var selectedRepo: RepoInfo? {
		let visible = visibleRepos
		return cursor.position(in: visible.count).map { visible[$0] }
	}

	func click(_ repo: RepoInfo) {
		if let index = visibleRepos.firstIndex(of: repo) { cursor.select(index) }
	}

	/// Adds the selected repo as `<repo>` or `<repo>-<postfix>`; the core rejects a name the workspace has, or one that leaves it, as the add's failure.
	func submit() {
		guard operation == nil, let repo = selectedRepo else { return }
		let dirName = AddRepoTarget.worktreeDirName(repo: repo.name, postfix: postfix)
		focus = .cancel
		operation = .start(
			title: "Adding: \(dirName)",
			request: .addRepo(workspacePath: target.workspacePath, sourceRepoPath: repo.path, worktreeDirName: dirName),
			store: store, close: close, onDone: { [onAdded] _ in onAdded() })
	}

	func cancel() {
		if operation == nil { close() }
	}

	/// `q` (outside a field) and a click on the backdrop: cancel the form; while the add runs, `q` goes to its progress.
	func dismiss() {
		if let operation { operation.closeOrCancel() } else { close() }
	}

	func cycleFocus(backward: Bool) {
		guard operation == nil else { return }
		let order = Focus.allCases
		let index = order.firstIndex(of: focus) ?? 0
		focus = order[(index + (backward ? order.count - 1 : 1)) % order.count]
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		if let operation { return operation.handleKey(key) }
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.escape:
			close()
		case KeyCode.returnKey, KeyCode.keypadEnter:
			submit()
		case KeyCode.downArrow where focus == .filter:
			cursor.move(by: 1, in: visibleRepos.count)
		case KeyCode.upArrow where focus == .filter:
			cursor.move(by: -1, in: visibleRepos.count)
		case KeyCode.space where !focus.isTextField:
			focus == .add ? submit() : close()
		default:
			// Typing goes to the focused field, and nothing beneath the dialog may see a key.
			return !focus.isTextField && key.isUnusedOnAPage
		}
		return true
	}
}

struct AddRepoDialogView: View {
	@Bindable var model: AddRepoDialogModel
	@FocusState private var field: AddRepoDialogModel.Focus?

	var body: some View {
		RepoDialogFrame(identifier: AddRepoDialogModel.layerName, onBackdropClick: model.cancel) {
			if let operation = model.operation {
				RepoOperationProgressView(phase: operation)
			} else {
				form
			}
		}
		.onChange(of: model.focus, initial: true) { _, focus in field = focus.isTextField ? focus : nil }
		.onChange(of: field) { _, field in
			if let field { model.focus = field }
		}
	}

	@ViewBuilder private var form: some View {
		Text("Add Repo").font(.title3.weight(.semibold))
		dialogFieldLabel("Repository")
		TextField("Filter repos…", text: $model.filter)
			.textFieldStyle(.roundedBorder)
			.focused($field, equals: .filter)
			.accessibilityIdentifier("add-repo-filter")
		let selected = model.selectedRepo
		RepoListView(
			repos: model.visibleRepos, active: selected, mode: .single,
			isSelected: { $0 == selected }, click: model.click, identifierPrefix: "add-repo-repo-")
		dialogFieldLabel("Name postfix (optional)")
		TextField("e.g. ref", text: $model.postfix)
			.textFieldStyle(.roundedBorder)
			.focused($field, equals: .postfix)
			.accessibilityIdentifier("add-repo-postfix")
		HStack(spacing: 10) {
			Spacer()
			DialogButton(title: "Cancel", isFocused: model.focus == .cancel, action: model.cancel)
			DialogButton(title: "Add Repo", isFocused: model.focus == .add, tint: .accentColor, action: model.submit)
		}
		.padding(.top, 4)
	}
}
