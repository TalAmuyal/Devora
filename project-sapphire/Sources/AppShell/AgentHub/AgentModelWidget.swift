import CoreBindings
import SwiftUI

/// What the model widget shows for an agent: the model its latest reply came from (the core's `ChatModel`), grayed
/// while that's pending or the agent is paused, with an orange glyph when the reply isn't from what it's set to.
struct AgentModelPresentation: Equatable {
	/// The model's name, else its ID as it is; "Pending" before a reply is known.
	let label: String
	/// Whether the label is a name the core gave the model, which the widget's width must always fit (an ID may be cut).
	let named: Bool
	let grayed: Bool
	let mismatch: Bool
	/// The widget's tooltip: the full ID and what the agent is set to.
	let tooltip: String
	/// The glyph's own tooltip, explaining a mismatch: what answered, what it's set to, and Claude Code's sentence.
	let glyphTooltip: String?
	/// What VoiceOver reads: the same, in words, since it can't see the glyph or its color.
	let accessibilityValue: String

	static let pending = "Pending"

	init(model: ChatModel?, paused: Bool) {
		let setTo = model.map { Self.setting($0) }
		guard let model, let latest = model.latest else {
			label = Self.pending
			named = false
			grayed = true
			mismatch = false
			tooltip = ["Waiting for the first reply", setTo.map { "set to \($0)" }].compactMap { $0 }.joined(separator: " · ")
			glyphTooltip = nil
			accessibilityValue = ["Pending", setTo.map { "set to \($0)" }].compactMap { $0 }.joined(separator: ", ")
			return
		}
		let name = model.label ?? latest
		label = name
		named = model.label != nil && model.label != latest
		grayed = paused
		mismatch = model.differs
		let reply = paused ? "Last reply: \(latest)" : latest
		tooltip = [reply, setTo.map { "set to \($0)" }, paused ? "paused" : nil].compactMap { $0 }.joined(separator: " · ")
		glyphTooltip = model.differs
			? (["Answered by \(name) (\(latest)), set to \(setTo ?? model.setTo)."] + [model.fallback].compactMap { $0 })
				.joined(separator: " ")
			: nil
		accessibilityValue = [
			name, model.differs ? "not the model it's set to, \(setTo ?? model.setTo)" : nil, paused ? "paused" : nil,
		].compactMap { $0 }.joined(separator: ", ")
	}

	/// What the agent is set to, with what that resolves to when it's an alias: "opus (claude-opus-5-5)".
	private static func setting(_ model: ChatModel) -> String {
		guard let resolved = model.setToResolved, resolved != model.setTo else { return model.setTo }
		return "\(model.setTo) (\(resolved))"
	}
}

/// The model widget in an agent's widget bar: a glyph and the model's label, in a fixed width that fits the glyph and the
/// widest of `names` (the models Claude Code offers the agent's profile), "Pending" and a named label, so it never moves the widgets
/// beside it; a longer label (an unnamed model's ID) is cut at its tail, with the full ID in the tooltip.
struct AgentModelWidget: View {
	let presentation: AgentModelPresentation
	let names: [String]

	/// The labels the width fits: the profile's model names, "Pending", and the label itself when it's a name, so a named
	/// model is never cut, even before the names arrive or when Claude Code couldn't be asked.
	static func reserved(names: [String], presentation: AgentModelPresentation) -> [String] {
		var reserved = names + [AgentModelPresentation.pending] + (presentation.named ? [presentation.label] : [])
		var seen = Set<String>()
		reserved.removeAll { !seen.insert($0).inserted }
		return reserved
	}

	var body: some View {
		HStack(spacing: 5) {
			Image(systemName: presentation.mismatch ? "arrow.triangle.swap" : "cpu")
				.foregroundStyle(presentation.mismatch ? AnyShapeStyle(.orange) : AnyShapeStyle(.secondary))
				.help(presentation.glyphTooltip ?? presentation.tooltip)
			ZStack(alignment: .leading) {
				ForEach(Self.reserved(names: names, presentation: presentation), id: \.self) { Text($0).hidden() }
			}
			.overlay(alignment: .leading) {
				Text(presentation.label).lineLimit(1).truncationMode(.tail)
			}
		}
		.font(.callout)
		.foregroundStyle(presentation.grayed ? .secondary : .primary)
		.fixedSize()
		.help(presentation.tooltip)
		.accessibilityElement(children: .combine)
		.accessibilityLabel("Agent model")
		.accessibilityValue(presentation.accessibilityValue)
		.accessibilityIdentifier("agent-model")
	}
}
