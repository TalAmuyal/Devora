import CoreBindings
import Foundation
import Testing

@testable import AppShell

@MainActor
struct AgentContextTests {
	let english = Locale(identifier: "en_US")

	@Test func aKnownContextShowsItsShareAndTheFullNumbers() {
		let context = ChatContext(usedTokens: 150534, windowTokens: 1_000_000, thresholdPercent: 80)
		let widget = AgentContextWidget(context: context, locale: english)
		#expect(widget.text == "Context 15.1%")
		#expect(widget.tooltip == "150,534 / 1,000,000 tokens · compaction at 80%")
	}

	@Test func theLocaleFormatsTheShareAndTheCountsAlike() {
		let context = ChatContext(usedTokens: 150534, windowTokens: 1_000_000, thresholdPercent: 80)
		let widget = AgentContextWidget(context: context, locale: Locale(identifier: "de_DE"))
		#expect(widget.text == "Context 15,1%")
		#expect(widget.tooltip == "150.534 / 1.000.000 tokens · compaction at 80%")
	}

	@Test func theCompactionThresholdFillsTheRing() {
		let halfway = ChatContext(usedTokens: 400_000, windowTokens: 1_000_000, thresholdPercent: 80)
		#expect(AgentContextWidget(context: halfway, locale: english).ringFraction == 0.5)
		let past = ChatContext(usedTokens: 900_000, windowTokens: 1_000_000, thresholdPercent: 80)
		#expect(AgentContextWidget(context: past, locale: english).ringFraction == 1)
	}

	@Test func usageBeyondTheWindowStopsAtAFullWindow() {
		let context = ChatContext(usedTokens: UInt64.max, windowTokens: 200000, thresholdPercent: 80)
		let widget = AgentContextWidget(context: context, locale: english)
		#expect(widget.fraction == 1)
		#expect(widget.ringFraction == 1)
		#expect(widget.text == "Context 100.0%")
	}

	@Test func unknownUsageSaysItIsWaiting() {
		let widget = AgentContextWidget(context: nil, locale: english)
		#expect(widget.fraction == nil)
		#expect(widget.ringFraction == 0)
		#expect(widget.text == "Context unknown")
		#expect(widget.tooltip == "Waiting for context usage")
	}

	@Test func anUnknownWindowKeepsTheReportedUsage() {
		let context = ChatContext(usedTokens: 150534, windowTokens: 0, thresholdPercent: 75)
		let widget = AgentContextWidget(context: context, locale: english)
		#expect(widget.fraction == nil)
		#expect(widget.ringFraction == 0)
		#expect(widget.text == "Context unknown")
		#expect(widget.tooltip == "150,534 tokens · window size not known yet")
	}
}
