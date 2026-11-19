import CoreBindings
import Store
import SwiftUI

/// Who a global hold is still waiting for, in a popover from the toolbar's pause status (`Surfaces.showPauseProgress`):
/// opened by clicking it, by a quit that has to wait, or by a shutdown that went wrong.
///
/// It stays out of the way of the agents' permission cards, which remain answerable beside it; dismissing it (Escape, a click elsewhere) only hides it, so a waiting quit stays armed, and only Cancel cancels it.
@MainActor
@Observable
final class PauseProgressModel {
	enum Mode: Equatable {
		/// Opened from the toolbar while the hold stops everyone.
		case pausing
		/// A quit waits for everyone to stop; it goes on by itself once they have.
		case quitting
		/// The shutdown is over, but it didn't go cleanly: what went wrong, before the app quits.
		case problems([String])
	}

	struct Action {
		let title: String
		var isDestructive = false
		/// The button Escape and Return don't reach: the safe one is the default.
		var isDefault = false
		let run: () -> Void
	}

	let mode: Mode
	let actions: [Action]

	init(mode: Mode, actions: [Action]) {
		self.mode = mode
		self.actions = actions
	}

	var isProblems: Bool {
		if case .problems = mode { true } else { false }
	}

	var title: String {
		switch mode {
		case .pausing: "Pausing Devora"
		case .quitting: "Quitting Devora"
		case .problems: "Devora didn't stop cleanly"
		}
	}

	static func describe(_ activity: AgentActivity) -> String {
		switch activity {
		case .turn: "running a turn"
		case .question: "waiting for your answer to a permission question"
		case .idle: "stopping"
		case .stopping: "stopping (its tab was closed)"
		}
	}
}

struct PauseProgressView: View {
	let model: PauseProgressModel
	let store: Store
	/// Activates the agent's tab, hiding the popover, so that its question can be answered.
	let show: (UInt64) -> Void

	var body: some View {
		VStack(alignment: .leading, spacing: 10) {
			Text(model.title).font(.headline)
			switch model.mode {
			case .problems(let problems):
				Text("Every agent has stopped, but not all of them cleanly:")
				ForEach(problems, id: \.self) { Text("• \($0)").textSelection(.enabled) }
			case .pausing, .quitting:
				agents
			}
			HStack {
				Spacer()
				ForEach(Array(model.actions.enumerated()), id: \.offset) { _, action in
					Button(action.title, role: action.isDestructive ? .destructive : nil, action: action.run)
						.keyboardShortcut(action.isDefault ? .defaultAction : nil)
				}
			}
			.padding(.top, 2)
		}
		.padding(16)
		.frame(width: 380, alignment: .leading)
		// Opaque, rather than the popover's translucent material, so that what's behind doesn't show through.
		.presentationBackground(Color(nsColor: .windowBackgroundColor))
		// The agents are stopped, so the problems' Quit is the only way on.
		.interactiveDismissDisabled(model.isProblems)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("pause-progress")
	}

	@ViewBuilder private var agents: some View {
		let running = store.runningAgents.running
		if running.isEmpty {
			Text(model.mode == .quitting ? "Every agent has stopped; quitting…" : "Every agent has stopped.")
				.foregroundStyle(.secondary)
		} else {
			Text(model.mode == .quitting
				? "Devora quits by itself once \(running.count == 1 ? "this agent has" : "these agents have") stopped cleanly."
				: "\(store.runningAgents.total - UInt32(running.count)) of \(store.runningAgents.total) agents have stopped.")
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)
			ForEach(running, id: \.name) { agent in
				HStack(alignment: .firstTextBaseline, spacing: 8) {
					VStack(alignment: .leading, spacing: 2) {
						Text(agent.name).bold()
						Text(PauseProgressModel.describe(agent.activity)).font(.callout).foregroundStyle(.secondary)
					}
					Spacer()
					if let tab = agent.tab {
						Button("Show") { show(tab) }
							.accessibilityIdentifier("pause-progress-show-\(agent.name)")
					}
				}
			}
		}
	}
}
