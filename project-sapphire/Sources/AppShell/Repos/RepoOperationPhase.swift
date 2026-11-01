import CoreBindings
import Layers
import Store
import SwiftUI

/// The progress phase the Add Repo and Clone Repo dialogs share, as Ember's `driveCreationDialog` drives it: the dialog's form gives way to the operation's progress, until it is done or cancelled, which closes the dialog, or fails, which leaves the failure up until the dialog is closed.
///
/// Escape, `q`, Space and its button cancel the operation while it runs, and close the dialog once it has failed.
@MainActor
struct RepoOperationPhase {
	let title: String
	let run: RepoOperationRun
	private let close: () -> Void

	/// Starts the operation; `close` closes the dialog, and `onDone` runs after it once the operation is done.
	static func start(
		title: String, request: RepoOperationRequest, store: Store,
		close: @escaping () -> Void, onDone: @escaping (CreatedWorkspace) -> Void
	) -> RepoOperationPhase {
		let run = store.startRepoOperation(request) { outcome in
			switch outcome {
			case .done(let result):
				close()
				onDone(result)
			case .cancelled, .notStarted:
				close()
			case .failed:
				break
			}
		}
		return RepoOperationPhase(title: title, run: run, close: close)
	}

	private init(title: String, run: RepoOperationRun, close: @escaping () -> Void) {
		self.title = title
		self.run = run
		self.close = close
	}

	var failure: String? { run.progress.failure }
	var action: String { failure == nil ? "Cancel" : "Close" }

	func closeOrCancel() {
		if failure == nil { run.cancel() } else { close() }
	}

	/// Nothing beneath the dialog may see a key meanwhile.
	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.escape, KeyCode.space:
			closeOrCancel()
			return true
		case KeyCode.returnKey, KeyCode.keypadEnter:
			return true
		default:
			return key.isUnusedOnAPage
		}
	}
}

/// The progress phase's content, in place of the dialog's form.
struct RepoOperationProgressView: View {
	let phase: RepoOperationPhase

	var body: some View {
		VStack(alignment: .leading, spacing: 16) {
			Text(phase.title).font(.title3.weight(.semibold))
			CreationProgressDetails(progress: phase.run.progress, failure: phase.failure)
			HStack {
				HStack(spacing: 4) {
					KeyCap("Esc")
					Text("dismiss").foregroundStyle(.secondary)
				}
				.font(.caption)
				Spacer()
				DialogButton(title: phase.action, isFocused: true, action: phase.closeOrCancel)
					.accessibilityIdentifier("repo-operation-action")
			}
		}
	}
}

/// A repo dialog's frame: the dialog over a dimmed backdrop, which cancels it when clicked while it shows its form.
struct RepoDialogFrame<Content: View>: View {
	let identifier: String
	let onBackdropClick: () -> Void
	@ViewBuilder let content: Content

	var body: some View {
		ZStack {
			Color.black.opacity(0.35)
				.contentShape(.rect)
				.onTapGesture(perform: onBackdropClick)
				.accessibilityHidden(true)
			VStack(alignment: .leading, spacing: 10) {
				content
			}
			.padding(20)
			.frame(width: 480)
			.background(.windowBackground, in: .rect(cornerRadius: 12))
			.shadow(radius: 24)
			.accessibilityElement(children: .contain)
			.accessibilityIdentifier(identifier)
		}
	}
}

func dialogFieldLabel(_ text: String) -> some View {
	Text(text).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
}
