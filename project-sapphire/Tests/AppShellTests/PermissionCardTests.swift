import AppKit
import CoreBindings
import Testing

@testable import AppShell

private func question(
	_ id: String = "q1", tool: String = "t2", rules: String? = nil, always: Bool = true, reason: String? = "Ask rule"
) -> PermissionQuestion {
	PermissionQuestion(
		requestId: id, toolName: "Bash", toolUseId: tool, inputJson: #"{"command":"custom-tool"}"#,
		decisionReason: reason, canAlwaysAllow: always, suggestedRulesJson: rules)
}

private func card(_ rules: String?) -> PermissionCard {
	PermissionCard(question: question(rules: rules), generation: 7)
}

private func entry(_ sequence: UInt64, _ content: TranscriptContent) -> TranscriptItem {
	TranscriptItem(sequence: sequence, cliSession: "s", parentToolUseId: nil, state: .complete, content: content)
}

private func reply(_ sequence: UInt64) -> TranscriptItem { entry(sequence, .assistant(text: "Reply \(sequence).")) }

private func toolRow(_ sequence: UInt64) -> TranscriptItem {
	entry(sequence, .tool(toolUseId: "t\(sequence)", name: "Bash", inputJson: "{}", verdict: nil, result: nil))
}

/// The fake CLI's suggestion, as Claude Code sends one.
private let localRule = #"[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"custom-tool"}],"behavior":"allow","destination":"localSettings"}]"#

struct PermissionCardRulesTests {
	@Test func aSuggestedRuleReadsAsASentenceWithEveryField() {
		#expect(card(localRule).rules == ["Allow Bash(custom-tool), saved to localSettings"])
	}

	@Test func eachKindOfUpdateReadsAsASentence() {
		let updates = #"""
		[{"type":"addRules","rules":[{"toolName":"WebFetch"},{"toolName":"Bash","ruleContent":"ls:*"}],"behavior":"deny","destination":"userSettings"},
		 {"type":"replaceRules","rules":[{"toolName":"Read"}],"behavior":"ask","destination":"projectSettings"},
		 {"type":"removeRules","rules":[{"toolName":"Edit"}],"behavior":"allow","destination":"session"},
		 {"type":"setMode","mode":"acceptEdits","destination":"session"},
		 {"type":"addDirectories","directories":["/tmp/a","/tmp/b"],"destination":"localSettings"}]
		"""#
		#expect(card(updates).rules == [
			"Deny WebFetch, Bash(ls:*), saved to userSettings",
			"Replace the ask rules with Read, saved to projectSettings",
			"Remove the allow rule Edit, saved to session",
			"Switch to the acceptEdits mode, saved to session",
			"Add /tmp/a, /tmp/b, saved to localSettings",
		])
	}

	@Test func whatItCantSayInFullStaysJSON() throws {
		// An unknown kind, and a known one with a field the sentence would leave out.
		let updates = #"[{"type":"futureKind","destination":"session"},{"type":"setMode","mode":"plan","destination":"session","scope":"x"}]"#
		let rules = try #require(card(updates).rules)
		#expect(rules.count == 2)
		#expect(rules[0].contains(#""type" : "futureKind""#))
		#expect(rules[1].contains(#""scope" : "x""#) && rules[1].contains(#""mode" : "plan""#))
		#expect(card("not json").rules == ["not json"])
	}

	@Test func noSuggestionsMeansNoRules() {
		#expect(card(nil).rules == nil)
		#expect(card("[]").rules == nil)
		#expect(card("  ").rules == nil)
	}

	@Test func theCardsTextHasTheToolInputReasonAndRules() {
		let text = card(localRule).plainText
		#expect(text == "Bash\n{\n  \"command\" : \"custom-tool\"\n}\nAsk rule\nAllow Bash(custom-tool), saved to localSettings")
		#expect(PermissionCard(question: question(reason: nil), generation: 1).reason == "Permission requested")
	}
}

struct TranscriptEntryLayoutTests {
	@Test func aCardFollowsItsToolRowOrElseGoesLast() {
		let items = [reply(1), toolRow(2), reply(3)]
		let entries = TranscriptEntry.layout(items, questions: [question("q1", tool: "t2"), question("q9", tool: "elsewhere")], generation: 4)
		#expect(entries.map(\.key) == [.item(1), .item(2), .card("q1"), .item(3), .card("q9")])
		guard case .card(let card) = entries[2] else { return #expect(Bool(false)) }
		#expect(card.generation == 4)
	}

	@Test func withoutQuestionsTheRowsAreTheItems() {
		let items = [reply(1), toolRow(2)]
		#expect(TranscriptEntry.layout(items, questions: [], generation: 0) == items.map(TranscriptEntry.item))
	}

	@Test func cardsChangeTheTableAsRows() {
		let items = [reply(1), toolRow(2)]
		let none = TranscriptEntry.layout(items, questions: [], generation: 1)
		let asked = TranscriptEntry.layout(items, questions: [question("q1", tool: "t2")], generation: 1)
		#expect(TranscriptChange(from: none, to: asked) == .updated(changed: [], removedFromTop: 0, appended: 1, removedFromBottom: 0))
		#expect(TranscriptChange(from: asked, to: none) == .updated(changed: [], removedFromTop: 0, appended: 0, removedFromBottom: 1))
		// A card between rows coming or going: the table loads again.
		let later = items + [reply(3)]
		let between = TranscriptEntry.layout(later, questions: [question("q1", tool: "t2")], generation: 1)
		#expect(TranscriptChange(from: TranscriptEntry.layout(later, questions: [], generation: 1), to: between) == .reloaded)
	}
}

@MainActor
struct TranscriptCardTests {
	private func shown(_ items: [TranscriptItem], questions: [PermissionQuestion]) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: nil)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply(items, questions: questions, generation: 3)
		return (window, view)
	}

	@Test func theBridgeCountsCardsAndReadsTheirText() throws {
		let (window, view) = shown([reply(1), toolRow(2)], questions: [question("q1", tool: "t2", rules: localRule)])
		defer { window.close() }
		let observation = view.observation()
		#expect(observation["cards"] as? Int == 1)
		#expect((observation["rows"] as? [[String: Any]])?.count == 2, "A card isn't an item row")
		#expect(try #require(observation["text"] as? String).contains("saved to localSettings"))
		view.apply([reply(1), toolRow(2)], questions: [], generation: 3)
		#expect(view.observation()["cards"] as? Int == 0)
	}

	@Test func aCardIsMeasuredAsARow() throws {
		let (window, view) = shown([reply(1), toolRow(2)], questions: [question("q1", tool: "t2", rules: localRule)])
		defer { window.close() }
		// Visible, so measured at once: taller than a collapsed row, with its input, rules and buttons.
		#expect(view.table.rect(ofRow: 2).height > 100)
		#expect(view.distanceFromBottom == 0, "Following the bottom keeps the card in view")
	}

	@Test func aCardOnTopSavesThePositionOfTheRowAboveIt() {
		var items = (1...40).map { reply(UInt64($0)) }
		items.insert(toolRow(100), at: 10)
		let (window, view) = shown(items, questions: [question("q1", tool: "t100")])
		defer { window.close() }
		let card = view.entries.firstIndex { $0.key == .card("q1") }!
		view.contentView.scroll(to: NSPoint(x: 0, y: view.table.rect(ofRow: card).minY + 5))
		view.reflectScrolledClipView(view.contentView)
		let anchor = view.visibleAnchor
		#expect(anchor?.sequence == 100, "The card's tool row, since the card won't be there when the position comes back")
		#expect(anchor.map { Int($0.offset) } == Int(view.table.rect(ofRow: card - 1).minY - view.contentView.bounds.minY))
	}
}
