import CoreBindings
import SwiftUI

/// Under the transcript while a turn runs: a spinner, the step the turn is on (the core's `ChatStep`), and how long it
/// has been on it, ticking once a second in the view from the step's start, in Devora's shorthand for a duration.
struct AgentTurnStatus: View {
	let step: ChatStep

	var body: some View {
		TimelineView(.periodic(from: .now, by: 1)) { context in
			let elapsed = Self.elapsed(since: step.sinceMs, at: context.date)
			HStack(spacing: 8) {
				ProgressView().controlSize(.small)
				Text(Self.label(step.activity)).foregroundStyle(.secondary)
				Text(elapsed).monospacedDigit().foregroundStyle(.tertiary)
				Spacer(minLength: 0)
			}
			.font(.callout)
			.lineLimit(1)
			.padding(.horizontal, 16).frame(height: 28)
			.accessibilityElement(children: .ignore)
			.accessibilityLabel("\(Self.label(step.activity)), \(elapsed)")
		}
	}

	static func label(_ activity: StepActivity) -> String {
		switch activity {
		case .waiting(let tool, _): "Waiting for your answer: \(tool)"
		case .running(let tool, _, let subagent): subagent ? "Sub-agent is running \(tool)" : "Running \(tool)"
		case .thinking(let subagent): subagent ? "Sub-agent is thinking" : "Thinking"
		case .writing(let subagent): subagent ? "Sub-agent is writing" : "Writing"
		case .working: "Working"
		}
	}

	/// The step's time so far, never negative: the core's clock and this one are the same machine's, but a step that
	/// starts just after this frame was dated would otherwise read below zero.
	static func elapsed(since sinceMs: UInt64, at date: Date) -> String {
		let now = date.timeIntervalSince1970 * 1000
		let seconds = UInt64(max(0, (now - Double(sinceMs)) / 1000))
		return (try? formatDuration(seconds: seconds)) ?? ""
	}
}
