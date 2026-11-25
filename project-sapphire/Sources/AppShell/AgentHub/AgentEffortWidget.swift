import CoreBindings
import SwiftUI

/// What the Effort widget shows and offers, from the core's `ChatEffort`: the level the agent runs at (Claude Code's
/// words, or "default"), the levels its model takes, and when a pick applies. A pick is stored with the agent at once; a
/// running agent's CLI takes it between turns, and a paused one runs it when it resumes.
struct AgentEffortPresentation: Equatable {
	static let defaultLabel = "default"
	static let noEffort = "—"

	/// What the widget reads: the level that runs, "default" for Claude Code's, "—" for a model that takes none.
	let label: String
	let grayed: Bool
	/// Orange: a pick waiting for the turn to end, a level the model runs as another, or one Claude Code refused.
	let marked: Bool
	/// The level stored, which the menu checks; nil is Claude Code's default.
	let setTo: String?
	/// The levels the menu offers after "Claude Code default", lowest first.
	let levels: [String]
	/// The menu's first, disabled line: why nothing can be picked, or what's different from what was set.
	let note: String?
	/// Whether a level can be picked now.
	let enabled: Bool
	/// Whether "Claude Code default" can be picked now: also for a model the list doesn't know, since it sets no level.
	let defaultEnabled: Bool
	let tooltip: String
	let accessibilityValue: String

	init(effort: ChatEffort?, phase: ChatPhase?, turnRunning: Bool, globallyPaused: Bool) {
		let setTo = effort?.setTo
		let levels = effort?.levels
		let takesNone = levels?.isEmpty == true
		let runsAs = effort?.runsAs
		self.setTo = setTo
		self.levels = levels ?? []
		let paused = phase == .paused || globallyPaused
		let unavailable: String? =
			switch phase {
			case .running, .paused: nil
			case .opening: "This agent is starting"
			case .pausing: "This agent is pausing"
			default: "This agent isn't running"
			}
		if takesNone {
			label = Self.noEffort
		} else {
			label = runsAs ?? setTo ?? Self.defaultLabel
		}
		grayed = takesNone || paused
		marked = !takesNone && (effort?.pending == true || runsAs != nil || effort?.notTaken != nil)
		enabled = unavailable == nil && levels != nil && !takesNone
		defaultEnabled = unavailable == nil && (levels != nil || effort?.modelUnknown == true)
		let set = setTo ?? "Claude Code default"
		if let unavailable {
			note = unavailable
		} else if effort?.modelUnknown == true {
			note = "Claude Code doesn't list this model, so its effort levels aren't known."
		} else if levels == nil {
			note = "Waiting for the levels Claude Code offers"
		} else if takesNone {
			note = "This model takes no effort level."
		} else if let refused = effort?.notTaken {
			note = "Claude Code didn't take \(set) (\(refused)). It's tried again at the next launch."
		} else if let runsAs, let setTo {
			note = "This model runs \(setTo) as \(runsAs)."
		} else if effort?.pending == true {
			note = "\(set) applies when this turn ends."
		} else if turnRunning {
			note = "A turn is running: a new level applies when it ends."
		} else if paused {
			note = "Paused: the level you pick applies when the agent resumes."
		} else {
			note = nil
		}
		tooltip = ["Effort: \(set)", runsAs.map { "runs as \($0)" }, note].compactMap { $0 }.joined(separator: " · ")
		accessibilityValue = [label, note].compactMap { $0 }.joined(separator: ", ")
	}
}

/// The Effort widget in an agent's widget bar: a gauge and the level, at a fixed width that fits every level Claude Code
/// has, opening a native menu of "Claude Code default" and the levels the agent's model takes, the stored one checked.
struct AgentEffortWidget: View {
	static let reserved = [AgentEffortPresentation.defaultLabel, AgentEffortPresentation.noEffort, "low", "medium", "high", "xhigh", "max"]

	let presentation: AgentEffortPresentation
	let onPick: (String?) -> Void

	var body: some View {
		Menu {
			if let note = presentation.note {
				Text(note)
				Divider()
			}
			// Kept on one condition: picking it makes Claude Code use its own default for the model, which the pinned-CLI
			// test `pinned_cli_changes_the_effort_between_turns_and_runs_an_unsupported_level_at_the_one_below` asserts.
			// Should that stop holding, this entry goes with it.
			choice("Claude Code default", checked: presentation.setTo == nil) { onPick(nil) }
				.disabled(!presentation.defaultEnabled)
			Divider()
			ForEach(presentation.levels, id: \.self) { level in
				choice(level, checked: presentation.setTo == level) { onPick(level) }
					.disabled(!presentation.enabled)
				.accessibilityIdentifier("agent-effort-\(level)")
			}
		} label: {
			HStack(spacing: 5) {
				Image(systemName: "gauge.with.dots.needle.67percent")
					.foregroundStyle(presentation.marked ? AnyShapeStyle(.orange) : AnyShapeStyle(.secondary))
				ZStack(alignment: .leading) {
					ForEach(Self.reserved, id: \.self) { Text($0).hidden() }
					Text(presentation.label)
						.foregroundStyle(
							presentation.marked
								? AnyShapeStyle(.orange) : AnyShapeStyle(presentation.grayed ? .secondary : .primary))
				}
				Image(systemName: "chevron.down").font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
			}
			.font(.callout)
			.contentShape(.rect)
			// On the label, which the button style keeps as one element; on the menu it would reach its inner button too.
			.accessibilityElement(children: .combine)
			.accessibilityLabel("Agent effort")
			.accessibilityValue(presentation.accessibilityValue)
			.accessibilityIdentifier("agent-effort")
		}
		// A plain button draws the label as written; the borderless menu style redraws it, dropping its colors.
		.menuStyle(.button)
		.buttonStyle(.plain)
		.menuIndicator(.hidden)
		.fixedSize()
		.help(presentation.tooltip)
	}

	/// A menu entry with the menu's own checkmark when it's the level stored (a toggle draws one; a button with a
	/// checkmark image doesn't): choosing it picks the level, whatever its state.
	private func choice(_ title: String, checked: Bool, pick: @escaping () -> Void) -> some View {
		Toggle(title, isOn: Binding(get: { checked }, set: { _ in pick() }))
	}
}
