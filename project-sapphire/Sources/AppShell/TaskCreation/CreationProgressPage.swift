import CoreBindings
import Store
import SwiftUI

/// A tab's task-creation progress, Ember's `TaskCreationProgress`: a page on the tab's own layer stack (`Surfaces.sync`), from the tab's opening until it has its workspace, so it covers only its tab.
///
/// Escape, `q` and its button cancel the creation while it runs, and close the tab once it has failed.
struct CreationProgressPage: View {
	static let layerName = "task-creation-progress"

	let store: Store
	let tab: UInt64
	/// Expanded by a failure, which the output almost always explains.
	@State private var isOutputOpen = false

	var body: some View {
		let state = CreationPageState(store: store, tab: tab)
		VStack(alignment: .leading, spacing: 16) {
			Text("Creating: \(state.title)").font(.title2.weight(.semibold))
			VStack(alignment: .leading, spacing: 8) {
				ForEach(Array(state.progress.steps.enumerated()), id: \.offset) { _, step in
					HStack(spacing: 10) {
						StepIndicator(state: step.state).frame(width: 14, height: 14)
						Text(step.label).fontWeight(step.state == .active ? .semibold : .regular)
					}
				}
			}
			DisclosureGroup("Output", isExpanded: $isOutputOpen) {
				ScrollView {
					Text(state.progress.output.joined(separator: "\n"))
						.font(.callout.monospaced())
						.textSelection(.enabled)
						.frame(maxWidth: .infinity, alignment: .leading)
						.padding(8)
				}
				.defaultScrollAnchor(.bottom)
				.frame(maxHeight: 260)
				.background(.quaternary.opacity(0.4), in: .rect(cornerRadius: 6))
			}
			if let failure = state.failure {
				Text(failure)
					.font(.callout.monospaced())
					.foregroundStyle(.red)
					.textSelection(.enabled)
					.padding(10)
					.frame(maxWidth: .infinity, alignment: .leading)
					.background(.red.opacity(0.08), in: .rect(cornerRadius: 6))
					.accessibilityIdentifier("creation-error")
			}
			Spacer(minLength: 0)
			HStack {
				HStack(spacing: 4) {
					KeyCap("Esc")
					Text("dismiss").foregroundStyle(.secondary)
				}
				.font(.caption)
				Spacer()
				Button(state.action) { state.closeOrCancel() }
					.accessibilityIdentifier("creation-action")
			}
		}
		.padding(32)
		.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
		.background(.windowBackground)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier(Self.layerName)
		.onChange(of: state.failure != nil, initial: true) { _, failed in
			if failed { isOutputOpen = true }
		}
	}
}

/// What a tab's progress page shows and does, from the store: the creation's progress, and its failure, from the progress or, for a creation the app stopped in the middle of, from the restored tab.
@MainActor
struct CreationPageState {
	let store: Store
	let tab: UInt64

	private var appTab: AppTab? { store.app.window.tabs.first { $0.id == tab } }

	var title: String { appTab?.title ?? "" }
	var progress: CreationProgress { store.creations[tab] ?? CreationProgress() }

	var failure: String? {
		if let failure = progress.failure { return failure }
		guard case .creationFailed(let reason)? = appTab?.session.kind else { return nil }
		return reason == "interrupted" ? "Interrupted: Devora Sapphire stopped before the workspace was ready." : reason
	}

	var action: String { failure == nil ? "Cancel" : "Close" }

	/// Cancels the creation while it runs (its tab closes once the work is undone), and closes the tab once it has failed.
	func closeOrCancel() {
		if failure == nil {
			store.dispatch(.cancelCreation(tab: tab))
		} else {
			store.dispatch(.closeTab(id: tab))
		}
	}
}

private struct StepIndicator: View {
	let state: CreationProgress.StepState

	var body: some View {
		switch state {
		case .active: ProgressView().controlSize(.small)
		case .done: StatusDotView(dot: .clean)
		case .failed: StatusDotView(dot: .error)
		}
	}
}

extension SessionKind {
	/// A tab creating its workspace, or whose creation failed: its progress page covers it.
	var isCreation: Bool {
		switch self {
		case .creatingWorkspace, .creationFailed: true
		case .shell, .workspace: false
		}
	}
}
