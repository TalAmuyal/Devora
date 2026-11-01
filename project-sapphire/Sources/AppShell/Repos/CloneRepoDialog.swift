import CoreBindings
import Layers
import Store
import SwiftUI

/// One open Clone Repo dialog, Ember's `CloneRepoDialog`: a repository URL with a live "Clones to:" hint, then the clone's progress in its place (`RepoOperationPhase`), as a modal layer over the window (`Surfaces.openCloneRepoDialog`).
///
/// Its keys are Ember's: Escape cancels and Enter clones, whatever has the focus, and `q` goes on to the stack's dismissal, which types in the field and cancels on a button.
/// Tab and Shift+Tab move the focus around its field and buttons without ever leaving it, and Space presses the focused button, whose focus it draws itself.
@MainActor
@Observable
final class CloneRepoDialogModel {
	static let layerName = "clone-repo-dialog"

	/// In Tab order, which is the order they are drawn in.
	enum Focus: CaseIterable {
		case url
		case cancel
		case clone
	}

	/// The profile the repo is cloned into.
	let profilePath: String
	var url = "" {
		didSet { destination = store.cloneDestination(profilePath: profilePath, url: url) }
	}
	/// Where the clone would go, from the core, which parses the URL as the clone will; nil while the URL isn't clonable.
	private(set) var destination: String?
	var focus = Focus.url {
		didSet { if focus != .url { clearFocus() } }
	}
	/// Once the clone has started, the dialog shows its progress instead of the form.
	private(set) var operation: RepoOperationPhase?

	@ObservationIgnored private let store: Store
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private let close: () -> Void
	@ObservationIgnored private let onCloned: (CreatedWorkspace) -> Void

	/// `close` closes the dialog; `onCloned` runs once the repo was cloned, with its path and name.
	init(
		profilePath: String, store: Store,
		clearFocus: @escaping () -> Void, close: @escaping () -> Void, onCloned: @escaping (CreatedWorkspace) -> Void
	) {
		self.profilePath = profilePath
		self.store = store
		self.clearFocus = clearFocus
		self.close = close
		self.onCloned = onCloned
	}

	var hint: String? { destination.map { "Clones to:  \($0)" } }

	/// Clones the URL, trimmed; with none, nothing happens, and one the core can't parse fails the clone.
	func submit() {
		let url = url.trimmingCharacters(in: .whitespacesAndNewlines)
		guard operation == nil, !url.isEmpty else { return }
		focus = .cancel
		operation = .start(
			title: "Cloning repo", request: .cloneRepo(profilePath: profilePath, url: url),
			store: store, close: close, onDone: onCloned)
	}

	func cancel() {
		if operation == nil { close() }
	}

	/// `q` outside the field: cancels the form; while the clone runs, it goes to its progress.
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
		case KeyCode.space where focus != .url:
			focus == .clone ? submit() : close()
		default:
			// Typing goes to the field, and nothing beneath the dialog may see a key.
			return focus != .url && key.isUnusedOnAPage
		}
		return true
	}
}

struct CloneRepoDialogView: View {
	@Bindable var model: CloneRepoDialogModel
	@FocusState private var isURLFocused: Bool

	var body: some View {
		RepoDialogFrame(identifier: CloneRepoDialogModel.layerName, onBackdropClick: model.cancel) {
			if let operation = model.operation {
				RepoOperationProgressView(phase: operation)
			} else {
				form
			}
		}
		.onChange(of: model.focus, initial: true) { _, focus in isURLFocused = focus == .url }
		.onChange(of: isURLFocused) { _, focused in
			if focused { model.focus = .url }
		}
	}

	@ViewBuilder private var form: some View {
		Text("Clone Repo").font(.title3.weight(.semibold))
		dialogFieldLabel("Repository URL")
		TextField("e.g. git@github.com:org/repo.git", text: $model.url)
			.textFieldStyle(.roundedBorder)
			.focused($isURLFocused)
			.accessibilityIdentifier("clone-repo-url")
		if let hint = model.hint {
			Text(hint)
				.font(.callout.monospaced())
				.foregroundStyle(.secondary)
				.textSelection(.enabled)
				.accessibilityIdentifier("clone-repo-hint")
		}
		HStack(spacing: 10) {
			Spacer()
			DialogButton(title: "Cancel", isFocused: model.focus == .cancel, action: model.cancel)
			DialogButton(title: "Clone", isFocused: model.focus == .clone, tint: .accentColor, action: model.submit)
		}
		.padding(.top, 4)
	}
}
