import CoreBindings
import SwiftUI

/// What the Context widget's popover shows and allows, from the chat's context and the agent's state: its compaction
/// threshold (this agent's own, or its profile's) and Compact Now. Both need a running agent, since its actor owns the
/// space's database; otherwise they are disabled with the reason.
struct AgentContextPresentation {
	/// The slider's range: below 30% an agent would compact nearly every turn. A lower configured value still applies, and
	/// shows clamped, with a note.
	static let thresholds: ClosedRange<Double> = 30...100
	static let thresholdStep: Double = 5

	let context: ChatContext?
	let phase: ChatPhase?
	let pauseRequested: Bool
	let globallyPaused: Bool
	let turnRunning: Bool
	var locale: Locale = .autoupdatingCurrent

	enum CompactAction: Equatable { case now, afterTurn, none }

	var threshold: UInt8 { context?.thresholdPercent ?? 0 }
	var defaultThreshold: UInt8 { context?.defaultThresholdPercent ?? 0 }
	/// Whether the agent has a threshold of its own stored, which a change to its profile's doesn't reach, even when
	/// the two are equal.
	var overridden: Bool { context?.ownThresholdPercent != nil }

	/// Why the controls can't act, or nil when they can.
	var unavailableReason: String? {
		if globallyPaused { return "Devora is paused" }
		switch phase {
		case .running: return pauseRequested ? "This agent is pausing" : nil
		case .pausing: return "This agent is pausing"
		case .paused: return "This agent is paused"
		default: return "This agent isn't running"
		}
	}

	var sliderValue: Double { min(max(Double(threshold), Self.thresholds.lowerBound), Self.thresholds.upperBound) }

	var thresholdNote: String {
		if threshold < UInt8(Self.thresholds.lowerBound) {
			return "Set to \(threshold)% in the config · the slider starts at \(Int(Self.thresholds.lowerBound))%"
		}
		return overridden ? "This agent only · profile default is \(defaultThreshold)%" : "The profile's default"
	}

	/// When the next compaction happens, in tokens once the window is known.
	var compactsAt: String {
		guard let context else { return "Waiting for context usage" }
		if context.compaction == .idle && context.windowTokens > 0
			&& context.usedTokens * 100 >= context.windowTokens * UInt64(context.thresholdPercent)
		{
			// The threshold is checked when a turn ends, so a lower one starts nothing by itself.
			return "Compacts after the next turn"
		}
		guard context.windowTokens > 0 else { return "Compacts at \(context.thresholdPercent)%" }
		let tokens = (context.windowTokens * UInt64(context.thresholdPercent) / 100).formatted(.number.locale(locale))
		return "Compacts at \(context.thresholdPercent)% · \(tokens) tokens"
	}

	var compactAction: CompactAction {
		guard unavailableReason == nil, let context, context.compaction == .idle, context.usedTokens > 0 else {
			return .none
		}
		return turnRunning ? .afterTurn : .now
	}

	var compactTitle: String {
		switch context?.compaction {
		case .underway: return "Compacting…"
		case .requested: return "Compacts After This Turn"
		default: return compactAction == .afterTurn ? "Compact After This Turn" : "Compact Now"
		}
	}

	var compactHelp: String {
		if let unavailableReason { return unavailableReason }
		switch context?.compaction {
		case .underway: return "A compaction is under way"
		case .requested: return "It starts when the current turn ends"
		default: break
		}
		if (context?.usedTokens ?? 0) == 0 { return "Nothing to compact yet" }
		return "Saves the agent's memory, then compacts: the same steps the threshold starts"
	}
}

/// The Context widget's popover: how full the context is, this agent's compaction threshold, and Compact Now.
struct AgentContextPopover: View {
	let widget: AgentContextWidget
	let presentation: AgentContextPresentation
	let onThreshold: (UInt8?) -> Void
	let onCompactNow: () -> Void
	/// The slider's position while it's dragged; the threshold changes only once it's released.
	@State private var dragged: Double?

	var body: some View {
		VStack(alignment: .leading, spacing: 14) {
			HStack(spacing: 14) {
				ZStack {
					ContextRing(fraction: widget.ringFraction, lineWidth: 6)
					Text(widget.fraction.map(widget.percent) ?? "–").font(.callout).monospacedDigit()
				}
				.frame(width: 56, height: 56)
				VStack(alignment: .leading, spacing: 3) {
					Text("Context").font(.headline)
					Text(widget.tokens).foregroundStyle(.secondary).monospacedDigit()
					Text(presentation.compactsAt).font(.caption).foregroundStyle(.secondary)
						.accessibilityIdentifier("agent-context-compacts-at")
				}
			}
			Divider()
			VStack(alignment: .leading, spacing: 6) {
				HStack {
					Text("Compact at")
					Spacer()
					Text("\(Int(dragged ?? presentation.sliderValue))%").monospacedDigit()
					Button("Reset") { onThreshold(nil) }
						.buttonStyle(.link)
						.disabled(!presentation.overridden || presentation.unavailableReason != nil)
						.accessibilityIdentifier("agent-compact-threshold-reset")
				}
				Slider(
					value: Binding(get: { dragged ?? presentation.sliderValue }, set: { dragged = $0 }),
					in: AgentContextPresentation.thresholds, step: AgentContextPresentation.thresholdStep
				) {
					Text("Compact at")
				} minimumValueLabel: {
					Text("\(Int(AgentContextPresentation.thresholds.lowerBound))%")
				} maximumValueLabel: {
					Text("\(Int(AgentContextPresentation.thresholds.upperBound))%")
				} onEditingChanged: { editing in
					guard !editing, let value = dragged else { return }
					dragged = nil
					if UInt8(value) != presentation.threshold { onThreshold(UInt8(value)) }
				}
				.labelsHidden()
				.disabled(presentation.unavailableReason != nil)
				.accessibilityIdentifier("agent-compact-threshold")
				Text(presentation.unavailableReason ?? presentation.thresholdNote)
					.font(.caption).foregroundStyle(.secondary)
			}
			.help(presentation.unavailableReason ?? "")
			Divider()
			VStack(alignment: .leading, spacing: 6) {
				Button(action: onCompactNow) {
					Label(presentation.compactTitle, systemImage: "arrow.down.right.and.arrow.up.left")
						.frame(maxWidth: .infinity)
				}
				.controlSize(.large)
				.buttonStyle(.borderedProminent)
				.disabled(presentation.compactAction == .none)
				.help(presentation.compactHelp)
				.accessibilityIdentifier("agent-compact-now")
				Text("Saves the agent's memory, then runs /compact: the same steps the threshold starts.")
					.font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
			}
		}
		.padding(16)
		.frame(width: 320)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("agent-context-popover")
	}
}
