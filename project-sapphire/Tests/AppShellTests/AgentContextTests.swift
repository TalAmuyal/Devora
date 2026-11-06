import CoreBindings
import Testing

@testable import AppShell

@MainActor
struct AgentContextTests {
	@Test func unknownWindowKeepsReportedUsageAndThreshold() {
		let bar = AgentContextBar(context: ChatContext(usedTokens: 3400, windowTokens: 0, thresholdPercent: 75))
		#expect(bar.fraction == 0)
		#expect(bar.summary.contains("window pending"))
		#expect(bar.thresholdLabel == "Compaction at 75%")
	}

	@Test func fillUsesTheReportedWindowAndClampsOverflow() {
		let context = ChatContext(usedTokens: 150000, windowTokens: 200000, thresholdPercent: 80)
		#expect(AgentContextBar(context: context).fraction == 0.75)
		let full = ChatContext(usedTokens: UInt64.max, windowTokens: 200000, thresholdPercent: 80)
		#expect(AgentContextBar(context: full).fraction == 1)
	}
}
