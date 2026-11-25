import CoreBindings
import Store
import SwiftUI

/// What the Model widget's popover offers: the models Claude Code offers the agent's profile, as every model field shows
/// them, narrowed by what's typed, with a note on when a pick takes effect. A pick is stored with the agent at once; a
/// running agent's CLI takes it between turns, and a paused one runs it when it resumes.
struct AgentModelPickerPresentation {
	let model: ChatModel?
	let phase: ChatPhase?
	let globallyPaused: Bool
	let turnRunning: Bool
	let suggestions: ModelSuggestions?
	/// Whether asking Claude Code for its models failed; opening the popover again asks again.
	let lookupFailed: Bool

	/// Why no model can be picked now, or nil when one can.
	var unavailableReason: String? {
		if suggestions == nil {
			return lookupFailed
				? "Claude Code couldn't be asked for its models. Close and open this again to retry."
				: "Asking Claude Code for its models…"
		}
		switch phase {
		case .running, .paused: return nil
		case .opening: return "This agent is starting"
		case .pausing: return "This agent is pausing"
		default: return "This agent isn't running"
		}
	}

	/// The line above the list: why nothing can be picked, that Claude Code refused the last pick, or when a pick applies.
	var note: String? {
		if let unavailableReason { return unavailableReason }
		if let model, let refused = model.changeNotTaken {
			return "Claude Code didn't take \(model.setTo) (\(refused)). The agent will try it again at its next launch."
		}
		if phase == .paused || globallyPaused { return "Paused: the model you pick runs when the agent resumes." }
		if turnRunning { return "A turn is running: the model you pick takes over when it ends." }
		return nil
	}

	/// Whether the note warns (Claude Code refused the pick, or couldn't be asked) rather than informs.
	var noteWarns: Bool { model?.changeNotTaken != nil || lookupFailed }

	/// What the agent is set to, which the list marks.
	var current: String? { model?.setTo }

	/// Where the highlight starts among `rows`: on what the agent is set to, else on the first.
	func startingHighlight(in rows: [ModelSuggestions.Row]) -> String? {
		(rows.first { $0.completion == current } ?? rows.first)?.completion
	}

	/// Where the highlight goes once `typed` changes: on the row it names exactly, else on the first.
	/// A full value typed exactly shows every row, as a model field shows them for the value it holds, so the first row
	/// would be another model.
	func highlight(afterTyping typed: String) -> String? {
		let rows = rows(typed: typed)
		return (rows.first { $0.completion == typed } ?? rows.first)?.completion
	}

	func rows(typed: String) -> [ModelSuggestions.Row] {
		suggestions?.rows(for: .agentModel, typed: typed) ?? []
	}
}

/// The Model widget's popover: a search field (focused when it opens), the aliases and the model IDs, and the note.
/// ↑/↓ move the highlight, Return picks it, Escape closes.
struct AgentModelPopover: View {
	static let width: CGFloat = 340

	let presentation: AgentModelPickerPresentation
	let onPick: (String) -> Void
	@State private var typed: String
	/// The row Return picks: what the agent is set to when the popover opens, then the match of what's typed.
	@State private var highlighted: String?
	@FocusState private var searching: Bool

	init(presentation: AgentModelPickerPresentation, typed: String, onPick: @escaping (String) -> Void) {
		self.presentation = presentation
		self.onPick = onPick
		_typed = State(initialValue: typed)
		_highlighted = State(initialValue: presentation.startingHighlight(in: presentation.rows(typed: typed)))
	}

	private var rows: [ModelSuggestions.Row] { presentation.rows(typed: typed) }

	var body: some View {
		VStack(alignment: .leading, spacing: 10) {
			if let note = presentation.note {
				Label(note, systemImage: presentation.noteWarns ? "exclamationmark.triangle" : "info.circle")
					.font(.callout)
					.foregroundStyle(presentation.noteWarns ? AnyShapeStyle(.orange) : AnyShapeStyle(.secondary))
					.fixedSize(horizontal: false, vertical: true)
					.accessibilityIdentifier("agent-model-note")
			}
			TextField("Search models", text: $typed)
				.textFieldStyle(.roundedBorder)
				.focused($searching)
				.disabled(presentation.unavailableReason != nil)
				.onKeyPress(.downArrow) { move(1) }
				.onKeyPress(.upArrow) { move(-1) }
				.onKeyPress(.return) {
					guard let highlighted else { return .ignored }
					onPick(highlighted)
					return .handled
				}
				.accessibilityIdentifier("agent-model-search")
			ScrollViewReader { scroller in
				ScrollView {
					LazyVStack(alignment: .leading, spacing: 0) {
						ForEach(Array(sections.enumerated()), id: \.offset) { index, section in
							if index > 0 { Divider().padding(.vertical, 4) }
							Text(section.title).font(.caption).foregroundStyle(.secondary)
								.padding(.horizontal, 8).padding(.vertical, 3)
							ForEach(section.rows) { row in
								ModelPickerRow(
									row: row, current: row.completion == presentation.current,
									highlighted: row.completion == highlighted
								)
								.onTapGesture { onPick(row.completion) }
								.id(row.completion)
							}
						}
						if presentation.suggestions == nil && !presentation.lookupFailed {
							ProgressView().controlSize(.small).frame(maxWidth: .infinity).padding(.top, 40)
								.accessibilityLabel("Asking Claude Code for its models")
						}
						if rows.isEmpty && presentation.unavailableReason == nil {
							Text("No model matches “\(typed)”").foregroundStyle(.secondary).padding(8)
						}
					}
				}
				.frame(height: 260)
				.onChange(of: highlighted) { _, row in
					if let row { scroller.scrollTo(row) }
				}
			}
			.disabled(presentation.unavailableReason != nil)
		}
		.padding(12)
		.frame(width: Self.width)
		.onAppear { searching = true }
		.onChange(of: typed) { highlighted = presentation.highlight(afterTyping: typed) }
		.onChange(of: presentation.suggestions) {
			// The list arrived while the popover was open.
			if highlighted == nil { highlighted = presentation.startingHighlight(in: rows) }
			searching = true
		}
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("agent-model-popover")
	}

	private var sections: [(title: String, rows: [ModelSuggestions.Row])] {
		[ModelSuggestions.Row.Section.aliases, .modelIDs].compactMap { section in
			let rows = rows.filter { $0.section == section }
			return rows.isEmpty ? nil : (section.rawValue, rows)
		}
	}

	private func move(_ step: Int) -> KeyPress.Result {
		let ids = rows.map(\.completion)
		guard !ids.isEmpty else { return .ignored }
		let at = highlighted.flatMap { ids.firstIndex(of: $0) }
		highlighted = ids[at.map { min(max($0 + step, 0), ids.count - 1) } ?? (step > 0 ? 0 : ids.count - 1)]
		return .handled
	}
}

/// One model in the popover: its label, a model's ID beside it, and a checkmark on what the agent is set to.
private struct ModelPickerRow: View {
	let row: ModelSuggestions.Row
	let current: Bool
	let highlighted: Bool

	var body: some View {
		HStack(spacing: 8) {
			Image(systemName: "checkmark").opacity(current ? 1 : 0).accessibilityHidden(true)
			Text(row.name).lineLimit(1)
			Spacer(minLength: 8)
			if row.section == .modelIDs {
				Text(row.modelID).font(.callout.monospaced()).foregroundStyle(highlighted ? .primary : .secondary).lineLimit(1)
			}
		}
		.padding(.horizontal, 8).padding(.vertical, 4)
		.background(highlighted ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 5))
		.contentShape(.rect)
		.accessibilityElement(children: .combine)
		.accessibilityAddTraits(current ? .isSelected : [])
		.accessibilityIdentifier("agent-model-row-\(row.completion)")
	}
}
