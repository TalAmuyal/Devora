import CoreBindings
import Foundation
import Testing

@testable import AppShell

@MainActor
struct AgentContextTests {
	let english = Locale(identifier: "en_US")

	@Test func aKnownContextShowsItsShareAndTheFullNumbers() {
		let context = ChatContext(usedTokens: 150534, windowTokens: 1_000_000, thresholdPercent: 80, defaultThresholdPercent: 80, compaction: .idle)
		let widget = AgentContextWidget(context: context, locale: english)
		#expect(widget.text == "Context 15.1%")
		#expect(widget.tooltip == "150,534 / 1,000,000 tokens · compaction at 80%")
	}

	@Test func theLocaleFormatsTheShareAndTheCountsAlike() {
		let context = ChatContext(usedTokens: 150534, windowTokens: 1_000_000, thresholdPercent: 80, defaultThresholdPercent: 80, compaction: .idle)
		let widget = AgentContextWidget(context: context, locale: Locale(identifier: "de_DE"))
		#expect(widget.text == "Context 15,1%")
		#expect(widget.tooltip == "150.534 / 1.000.000 tokens · compaction at 80%")
	}

	@Test func theCompactionThresholdFillsTheRing() {
		let halfway = ChatContext(usedTokens: 400_000, windowTokens: 1_000_000, thresholdPercent: 80, defaultThresholdPercent: 80, compaction: .idle)
		#expect(AgentContextWidget(context: halfway, locale: english).ringFraction == 0.5)
		let past = ChatContext(usedTokens: 900_000, windowTokens: 1_000_000, thresholdPercent: 80, defaultThresholdPercent: 80, compaction: .idle)
		#expect(AgentContextWidget(context: past, locale: english).ringFraction == 1)
	}

	@Test func usageBeyondTheWindowStopsAtAFullWindow() {
		let context = ChatContext(usedTokens: UInt64.max, windowTokens: 200000, thresholdPercent: 80, defaultThresholdPercent: 80, compaction: .idle)
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
		let context = ChatContext(usedTokens: 150534, windowTokens: 0, thresholdPercent: 75, defaultThresholdPercent: 75, compaction: .idle)
		let widget = AgentContextWidget(context: context, locale: english)
		#expect(widget.fraction == nil)
		#expect(widget.ringFraction == 0)
		#expect(widget.text == "Context unknown")
		#expect(widget.tooltip == "150,534 tokens · window size not known yet")
	}

	func context(
		used: UInt64 = 300_000, threshold: UInt8 = 65, profile: UInt8 = 80, compaction: CompactionProgress = .idle
	) -> ChatContext {
		ChatContext(
			usedTokens: used, windowTokens: 1_000_000, thresholdPercent: threshold, defaultThresholdPercent: profile,
			compaction: compaction)
	}

	func presentation(
		_ context: ChatContext?, phase: ChatPhase = .running, pauseRequested: Bool = false, globallyPaused: Bool = false,
		turnRunning: Bool = false
	) -> AgentContextPresentation {
		AgentContextPresentation(
			context: context, phase: phase, pauseRequested: pauseRequested, globallyPaused: globallyPaused,
			turnRunning: turnRunning, locale: english)
	}

	@Test func anAgentsOwnThresholdShowsBesideItsProfilesInTheTooltipAndTheNote() {
		let widget = AgentContextWidget(context: context(), locale: english)
		#expect(widget.overridden)
		#expect(widget.tooltip == "300,000 / 1,000,000 tokens · compaction at 65% for this agent (profile: 80%)")
		#expect(presentation(context()).thresholdNote == "This agent only · profile default is 80%")
		#expect(presentation(context(threshold: 80)).thresholdNote == "The profile's default")
		#expect(!AgentContextWidget(context: context(threshold: 80), locale: english).overridden)
	}

	@Test func aThresholdUnderTheSlidersRangeShowsClampedAndSaysWhy() {
		let low = presentation(context(threshold: 10, profile: 10))
		#expect(low.sliderValue == 30)
		#expect(low.thresholdNote == "Set to 10% in the config · the slider starts at 30%")
	}

	@Test func itSaysWhenTheNextCompactionHappens() {
		#expect(presentation(context()).compactsAt == "Compacts at 65% · 650,000 tokens")
		// Lowered under the usage on an idle agent: nothing starts until a turn ends.
		#expect(presentation(context(used: 700_000)).compactsAt == "Compacts after the next turn")
	}

	@Test func compactNowStartsNowOrAfterTheRunningTurn() {
		let idle = presentation(context())
		#expect(idle.compactAction == .now)
		#expect(idle.compactTitle == "Compact Now")
		let busy = presentation(context(), turnRunning: true)
		#expect(busy.compactAction == .afterTurn)
		#expect(busy.compactTitle == "Compact After This Turn")
	}

	@Test func aCompactionAskedForOrUnderWayCanNotBeAskedForAgain() {
		let requested = presentation(context(compaction: .requested), turnRunning: true)
		#expect(requested.compactAction == .none)
		#expect(requested.compactTitle == "Compacts After This Turn")
		let underway = presentation(context(compaction: .underway), turnRunning: true)
		#expect(underway.compactAction == .none)
		#expect(underway.compactTitle == "Compacting…")
		#expect(underway.compactHelp == "A compaction is under way")
	}

	@Test func anAgentThatIsntRunningDisablesBothWithTheReason() {
		for (state, reason) in [
			(presentation(context(), phase: .paused), "This agent is paused"),
			(presentation(context(), pauseRequested: true), "This agent is pausing"),
			(presentation(context(), globallyPaused: true), "Devora is paused"),
			(presentation(context(), phase: .failed), "This agent isn't running"),
		] {
			#expect(state.unavailableReason == reason)
			#expect(state.compactAction == .none)
			#expect(state.compactHelp == reason)
		}
		#expect(presentation(context()).unavailableReason == nil)
	}

	@Test func anEmptyConversationHasNothingToCompact() {
		let empty = presentation(context(used: 0))
		#expect(empty.compactAction == .none)
		#expect(empty.compactHelp == "Nothing to compact yet")
	}
}
