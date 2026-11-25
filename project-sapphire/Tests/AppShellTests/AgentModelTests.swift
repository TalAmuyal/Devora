import CoreBindings
import Foundation
import Store
import Testing
import TranscriptKit

@testable import AppShell

@MainActor
struct AgentModelTests {
	func model(_ latest: String?, label: String? = nil, differs: Bool = false, fallback: String? = nil) -> ChatModel {
		ChatModel(
			latest: latest, label: label, setTo: "opus", setToResolved: "claude-opus-5-5", differs: differs,
			fallback: fallback, awaitingChange: false, changeNotTaken: nil)
	}

	@Test func beforeAReplyIsKnownTheWidgetIsPendingAndGrayed() {
		let shown = AgentModelPresentation(model: model(nil), paused: false)
		#expect(shown.label == "Pending")
		#expect(shown.grayed && !shown.mismatch)
		#expect(shown.tooltip == "Waiting for the first reply · set to opus (claude-opus-5-5)")
		#expect(AgentModelPresentation(model: nil, paused: false).label == "Pending")
	}

	@Test func aReplyFromTheModelSetShowsItsNameAndItsIDOnHover() {
		let shown = AgentModelPresentation(model: model("claude-opus-5-5", label: "Opus 5.5"), paused: false)
		#expect(shown.label == "Opus 5.5")
		#expect(!shown.grayed && !shown.mismatch && shown.glyphTooltip == nil)
		#expect(shown.tooltip == "claude-opus-5-5 · set to opus (claude-opus-5-5)")
		#expect(shown.accessibilityValue == "Opus 5.5")
	}

	@Test func aMismatchShowsOnlyInTheGlyphWhichExplainsItAndVoiceOverSaysItInWords() {
		let sentence = "Switched to Sonnet 5.5 due to high demand for Opus 5.5"
		let shown = AgentModelPresentation(
			model: model("claude-sonnet-5-5", label: "Sonnet 5.5", differs: true, fallback: sentence), paused: false)
		#expect(shown.label == "Sonnet 5.5", "no \", set to\" in the label")
		#expect(shown.mismatch)
		#expect(
			shown.glyphTooltip
				== "Answered by Sonnet 5.5 (claude-sonnet-5-5), set to opus (claude-opus-5-5). \(sentence)")
		#expect(shown.accessibilityValue == "Sonnet 5.5, not the model it's set to, opus (claude-opus-5-5)")
	}

	@Test func aPausedAgentShowsItsLastReplysModelGrayed() {
		let shown = AgentModelPresentation(model: model("claude-opus-5-5", label: "Opus 5.5"), paused: true)
		#expect(shown.label == "Opus 5.5" && shown.grayed)
		#expect(shown.tooltip == "Last reply: claude-opus-5-5 · set to opus (claude-opus-5-5) · paused")
	}

	@Test func aModelWithNoNameShowsItsIDAsItIs() {
		let shown = AgentModelPresentation(
			model: model("claude-sonnet-4-5-20250929", label: "claude-sonnet-4-5-20250929"), paused: false)
		#expect(shown.label == "claude-sonnet-4-5-20250929")
	}

	func changeRow(_ detail: String, text: String) -> CoreBindings.TranscriptItem {
		CoreBindings.TranscriptItem(
			sequence: 3, cliSession: "cli", parentToolUseId: nil, state: .complete,
			content: .system(kind: .modelChanged, text: text, detailJson: detail), createdAtMs: nil)
	}

	/// The adapted row's text, and its spoken form from its detail.
	func adapted(_ item: CoreBindings.TranscriptItem, names: [String: String]) -> (String, String?) {
		adapted(item, name: { names[$0] })
	}

	func adapted(_ item: CoreBindings.TranscriptItem, name: (String) -> String?) -> (String, String?) {
		guard case .system(.modelChanged, let text, let detailJson) = TranscriptKit.TranscriptItem(item, name: name).content
		else { return ("", nil) }
		let detail = try? JSONSerialization.jsonObject(with: Data(detailJson.utf8)) as? [String: Any]
		return (text, detail?["spoken"] as? String)
	}

	@Test func aModelChangeRowShowsTheModelsNamesAndSpeaksWithoutTheArrow() {
		let core = changeRow(
			#"{"from":"claude-sonnet-5-5","to":"claude-opus-5-5","fallback":false}"#,
			text: "Model changed: claude-sonnet-5-5 → claude-opus-5-5")
		let names = ["claude-sonnet-5-5": "Sonnet 5.5", "claude-opus-5-5": "Opus 5.5"]
		let (text, spoken) = adapted(core, names: names)
		#expect(text == "Model changed: Sonnet 5.5 → Opus 5.5")
		#expect(spoken == "Model changed from Sonnet 5.5 to Opus 5.5")
		#expect(adapted(core, names: [:]).0 == "Model changed: claude-sonnet-5-5 → claude-opus-5-5", "no name keeps the ID")
	}

	@Test func aFallbackRowKeepsClaudeCodesSentence() {
		let sentence = "Switched to Sonnet 5.5 due to high demand for Opus 5.5"
		let core = changeRow(#"{"from":"claude-opus-5-5","to":"claude-sonnet-5-5","fallback":true}"#, text: sentence)
		let (text, spoken) = adapted(core, names: ["claude-opus-5-5": "Opus 5.5"])
		#expect(text == sentence && spoken == sentence)
	}

	@Test func aChangeRowNamesADatedSnapshotByItsUndatedModelAsTheWidgetDoes() {
		let list = AgentModelList(
			cliVersion: "2.1.326", aliases: [],
			modelIds: [
				ModelRow(value: "claude-sonnet-4-5", label: "Sonnet 4.5", modelId: "claude-sonnet-4-5"),
				ModelRow(value: "claude-opus-5-5", label: "Opus 5.5", modelId: "claude-opus-5-5"),
			])
		let suggestions = ModelSuggestions(list)
		let core = changeRow(
			#"{"from":"claude-opus-5-5","to":"claude-sonnet-4-5-20250929","fallback":false}"#,
			text: "Model changed: claude-opus-5-5 → claude-sonnet-4-5-20250929")
		#expect(adapted(core, name: { suggestions.name(of: $0) }).0 == "Model changed: Opus 5.5 → Sonnet 4.5")
	}

	@Test func theWidgetFitsANamedModelBeforeTheProfilesNamesArrive() {
		let named = AgentModelPresentation(model: model("claude-sonnet-5-5", label: "Sonnet 5.5"), paused: false)
		#expect(AgentModelWidget.reserved(names: [], presentation: named) == ["Pending", "Sonnet 5.5"])
		let unnamed = AgentModelPresentation(
			model: model("claude-sonnet-4-5-20250929", label: "claude-sonnet-4-5-20250929"), paused: false)
		#expect(AgentModelWidget.reserved(names: ["Opus 5.5"], presentation: unnamed) == ["Opus 5.5", "Pending"], "an ID may be cut")
	}

	func picked(awaiting: Bool, notTaken: String? = nil) -> ChatModel {
		ChatModel(
			latest: "claude-opus-5-5", label: "Sonnet 5.5", setTo: "sonnet", setToResolved: "claude-sonnet-5-5", differs: true,
			fallback: nil, awaitingChange: awaiting, changeNotTaken: notTaken)
	}

	@Test func aModelJustPickedShowsInOrangeUntilItAnswersAndSaysWhyOnTheGlyph() {
		let shown = AgentModelPresentation(model: picked(awaiting: true), paused: false)
		#expect(shown.label == "Sonnet 5.5" && shown.changed && shown.mismatch)
		#expect(shown.glyphTooltip == "Set to Sonnet 5.5; no reply from it yet.")
		#expect(shown.tooltip == "Set to sonnet (claude-sonnet-5-5) · last reply from claude-opus-5-5")
		#expect(shown.accessibilityValue == "Sonnet 5.5, no reply from it yet")
		let answered = AgentModelPresentation(
			model: ChatModel(
				latest: "claude-sonnet-5-5", label: "Sonnet 5.5", setTo: "sonnet", setToResolved: "claude-sonnet-5-5",
				differs: false, fallback: nil, awaitingChange: false, changeNotTaken: nil), paused: false)
		#expect(!answered.changed && !answered.mismatch, "its first reply ends the mark")
	}

	@Test func aPickClaudeCodeRefusedNamesTheModelStillAnsweringAndSaysWhatWasRefused() {
		// What the core sends once Claude Code refused "sonnet": the pick stays stored, nothing awaits it, and the label
		// names the model still answering.
		let refused = ChatModel(
			latest: "claude-opus-5-5", label: "Opus 5.5", setTo: "sonnet", setToResolved: "claude-sonnet-5-5", differs: true,
			fallback: nil, awaitingChange: false, changeNotTaken: "Model 'sonnet' not found")
		let shown = AgentModelPresentation(model: refused, paused: false)
		#expect(shown.label == "Opus 5.5" && !shown.changed && shown.mismatch, "plain: the old model, with the glyph's mark")
		#expect(
			shown.glyphTooltip
				== "Claude Code didn't take sonnet (Model 'sonnet' not found). The agent will try it again at its next launch.")
		#expect(shown.accessibilityValue == "Opus 5.5, Claude Code didn't take sonnet")
		#expect(picker(refused).note?.hasPrefix("Claude Code didn't take sonnet") == true)

		// Refused before the agent's first reply: still Pending, with the same mark.
		let early = ChatModel(
			latest: nil, label: nil, setTo: "sonnet", setToResolved: "claude-sonnet-5-5", differs: true, fallback: nil,
			awaitingChange: false, changeNotTaken: "Model 'sonnet' not found")
		let pending = AgentModelPresentation(model: early, paused: false)
		#expect(pending.label == "Pending" && pending.grayed && pending.mismatch)
		#expect(pending.accessibilityValue == "Pending, Claude Code didn't take sonnet")
	}

	func picker(
		_ model: ChatModel?, phase: ChatPhase = .running, turn: Bool = false, listed: Bool = true, failed: Bool = false,
		globallyPaused: Bool = false
	) -> AgentModelPickerPresentation {
		let list = AgentModelList(
			cliVersion: "2.1.326",
			aliases: [ModelRow(value: "opus", label: "Opus (currently 5.5)", modelId: "claude-opus-5-5")],
			modelIds: [ModelRow(value: "claude-opus-5-5", label: "Opus 5.5", modelId: "claude-opus-5-5")])
		return AgentModelPickerPresentation(
			model: model, phase: phase, globallyPaused: globallyPaused, turnRunning: turn,
			suggestions: listed ? ModelSuggestions(list) : nil, lookupFailed: failed)
	}

	@Test func typingAModelsFullIDHighlightsThatModelThoughEveryRowShows() {
		let shown = picker(model("claude-sonnet-5-5", label: "Sonnet 5.5"))
		#expect(shown.rows(typed: "claude-opus-5-5").count == 2, "an exact value shows every row")
		#expect(shown.highlight(afterTyping: "claude-opus-5-5") == "claude-opus-5-5")
		#expect(shown.highlight(afterTyping: "opus") == "opus")
		#expect(shown.highlight(afterTyping: "op") == "opus", "else the first match")
	}

	@Test func thePopoverSaysWhenAPickTakesEffectOrWhyNoneCanBeMade() {
		let opus = model("claude-opus-5-5", label: "Opus 5.5")
		#expect(picker(opus).note == nil && picker(opus).unavailableReason == nil, "idle: it applies at once")
		#expect(picker(opus, turn: true).note == "A turn is running: the model you pick takes over when it ends.")
		#expect(picker(opus, phase: .paused).note == "Paused: the model you pick runs when the agent resumes.")
		#expect(picker(opus, phase: .paused, globallyPaused: true).unavailableReason == nil, "a held agent can be changed")
		#expect(picker(opus, phase: .pausing).unavailableReason == "This agent is pausing")
		#expect(picker(opus, phase: .opening).unavailableReason == "This agent is starting")
		#expect(picker(opus, listed: false).unavailableReason == "Asking Claude Code for its models…")
		#expect(picker(opus, listed: false, failed: true).unavailableReason?.hasPrefix("Claude Code couldn't be asked") == true)
		#expect(picker(opus).current == "opus", "the list marks what the agent is set to")
		#expect(picker(opus).rows(typed: "").map(\.completion) == ["opus", "claude-opus-5-5"])
	}
}
