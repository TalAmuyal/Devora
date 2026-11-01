import CoreBindings
import Layers
import SwiftUI

/// One open New Task dialog, Ember's `NewTaskDialog`: a title and a filterable multi-select list of the profile's repos, as a modal layer over the Workspace Hub (`Surfaces.openNewTaskDialog`).
///
/// Its keys are Ember's: Escape cancels and Enter creates, whatever has the focus, except that Enter toggles the active repo while the filter has it, where ↑ and ↓ move the active repo; `q` goes on to the stack's dismissal, which types in a field and cancels on a button.
/// Tab and Shift+Tab move the focus around its fields and buttons without ever leaving it; it draws its buttons' focus itself, as the confirmation dialog does.
@MainActor
@Observable
final class NewTaskDialogModel {
	static let layerName = "new-task-dialog"

	/// In Tab order, which is the order they are drawn in.
	enum Focus: CaseIterable {
		case title
		case cloneRepo
		case filter
		case cancel
		case create

		var isTextField: Bool { self == .title || self == .filter }
	}

	/// The repos the profile has, in its order.
	private(set) var repos: [RepoInfo]
	/// Duplicating a task: its repos are selected and its title is filled in.
	let isDuplicating: Bool
	var title: String
	/// A new filter moves the cursor to its first match; SwiftUI writes the field's same text back as it leaves the field, which moves nothing.
	var filter = "" {
		didSet { if filter != oldValue { cursor.select(0) } }
	}
	var focus = Focus.title {
		didSet { if !focus.isTextField { clearFocus() } }
	}
	/// The paths of the selected repos.
	private(set) var selected: Set<String>
	private(set) var cursor = ListCursor()

	/// Opens the Clone Repo dialog over this one; `reposChanged` follows a clone.
	@ObservationIgnored var cloneRepo: () -> Void = {}
	@ObservationIgnored private let onCreate: (_ title: String, _ repoPaths: [String]) -> Void
	@ObservationIgnored private let onCancel: () -> Void
	@ObservationIgnored private let clearFocus: () -> Void

	/// `onCreate` and `onCancel` close the dialog; at most one of them runs.
	init(
		repos: [RepoInfo], title: String, selected: Set<String>, isDuplicating: Bool,
		clearFocus: @escaping () -> Void,
		onCreate: @escaping (_ title: String, _ repoPaths: [String]) -> Void, onCancel: @escaping () -> Void
	) {
		self.repos = repos
		self.title = title
		self.selected = selected
		self.isDuplicating = isDuplicating
		self.clearFocus = clearFocus
		self.onCreate = onCreate
		self.onCancel = onCancel
	}

	var heading: String { isDuplicating ? "Duplicate Task" : "New Task" }

	/// The repos whose name contains the filter, ignoring case.
	var visibleRepos: [RepoInfo] { repos.matching(filter) }

	/// The row Enter toggles while the filter has the focus.
	var activeRepo: RepoInfo? {
		let visible = visibleRepos
		return cursor.position(in: visible.count).map { visible[$0] }
	}

	func toggle(_ repo: RepoInfo) {
		if selected.remove(repo.path) == nil { selected.insert(repo.path) }
	}

	/// A click on a row toggles it and makes it the active one, as Ember's does.
	func click(_ repo: RepoInfo) {
		toggle(repo)
		if let index = visibleRepos.firstIndex(of: repo) { cursor.select(index) }
	}

	/// Creates the task from the title, trimmed, and the selected repos; with no title, nothing happens.
	func create() {
		let title = title.trimmingCharacters(in: .whitespacesAndNewlines)
		guard !title.isEmpty else { return }
		onCreate(title, repos.map(\.path).filter(selected.contains))
	}

	func cancel() { onCancel() }

	/// After a clone: the profile's repos, listed again, with the clone selected too, as Ember's `refreshAfterClone` does; the title and the selection are kept.
	func reposChanged(_ repos: [RepoInfo], selecting path: String) {
		self.repos = repos
		selected = selected.intersection(repos.map(\.path))
		selected.insert(path)
	}

	/// Without repos, there is no filter to focus.
	func cycleFocus(backward: Bool) {
		let order = Focus.allCases.filter { $0 != .filter || !repos.isEmpty }
		let index = order.firstIndex(of: focus) ?? 0
		focus = order[(index + (backward ? order.count - 1 : 1)) % order.count]
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.escape:
			cancel()
		case KeyCode.returnKey, KeyCode.keypadEnter:
			if focus == .filter {
				if let activeRepo { toggle(activeRepo) }
			} else {
				create()
			}
		case KeyCode.downArrow where focus == .filter:
			cursor.move(by: 1, in: visibleRepos.count)
		case KeyCode.upArrow where focus == .filter:
			cursor.move(by: -1, in: visibleRepos.count)
		case KeyCode.space where !focus.isTextField:
			switch focus {
			case .create: create()
			case .cloneRepo: cloneRepo()
			case .title, .filter, .cancel: cancel()
			}
		default:
			// Typing goes to the focused field, and nothing beneath the dialog may see a key.
			return !focus.isTextField && key.isUnusedOnAPage
		}
		return true
	}
}

/// The dialog over a dimmed backdrop, which cancels it when clicked.
struct NewTaskDialogView: View {
	@Bindable var model: NewTaskDialogModel
	@FocusState private var field: NewTaskDialogModel.Focus?

	var body: some View {
		ZStack {
			Color.black.opacity(0.35)
				.contentShape(.rect)
				.onTapGesture { model.cancel() }
				.accessibilityHidden(true)
			VStack(alignment: .leading, spacing: 10) {
				Text(model.heading).font(.title3.weight(.semibold))
				fieldLabel("Title")
				TextField("e.g. Fix login bug", text: $model.title)
					.textFieldStyle(.roundedBorder)
					.focused($field, equals: .title)
					.accessibilityIdentifier("new-task-title")
				HStack {
					fieldLabel("Repositories")
					Spacer()
					DialogButton(title: "+ Clone Repo", isFocused: model.focus == .cloneRepo, action: model.cloneRepo)
						.accessibilityIdentifier("new-task-clone-repo")
				}
				if model.isDuplicating {
					Text("Pre-selected repos are duplicated at their current commit; added repos use the latest commit.")
						.font(.callout)
						.foregroundStyle(.secondary)
				}
				if model.repos.isEmpty {
					Text("No repos in this profile yet — clone one to add it.")
						.font(.callout)
						.foregroundStyle(.secondary)
				} else {
					TextField("Filter repos…", text: $model.filter)
						.textFieldStyle(.roundedBorder)
						.focused($field, equals: .filter)
						.accessibilityIdentifier("new-task-filter")
					RepoListView(
						repos: model.visibleRepos, active: model.activeRepo, mode: .multi,
						isSelected: { model.selected.contains($0.path) }, click: model.click,
						identifierPrefix: "new-task-repo-")
				}
				HStack(spacing: 10) {
					Spacer()
					DialogButton(title: "Cancel", isFocused: model.focus == .cancel, action: model.cancel)
					DialogButton(title: "Create", isFocused: model.focus == .create, tint: .accentColor, action: model.create)
				}
				.padding(.top, 4)
			}
			.padding(20)
			.frame(width: 480)
			.background(.windowBackground, in: .rect(cornerRadius: 12))
			.shadow(radius: 24)
			.accessibilityElement(children: .contain)
			.accessibilityIdentifier(NewTaskDialogModel.layerName)
		}
		.onChange(of: model.focus, initial: true) { _, focus in field = focus.isTextField ? focus : nil }
		.onChange(of: field) { _, field in
			if let field { model.focus = field }
		}
	}

	private func fieldLabel(_ text: String) -> some View {
		Text(text).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
	}
}
