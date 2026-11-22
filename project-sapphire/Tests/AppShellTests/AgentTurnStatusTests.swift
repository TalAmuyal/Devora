import CoreBindings
import Foundation
import Testing

@testable import AppShell

struct AgentTurnStatusTests {
	@Test func theLineNamesTheStep() {
		#expect(AgentTurnStatus.label(.waiting(tool: "Bash", requestId: "q1")) == "Waiting for your answer: Bash")
		#expect(AgentTurnStatus.label(.running(tool: "Read", toolUseId: "t1", subagent: false)) == "Running Read")
		#expect(AgentTurnStatus.label(.thinking(subagent: false)) == "Thinking")
		#expect(AgentTurnStatus.label(.writing(subagent: false)) == "Writing")
		#expect(AgentTurnStatus.label(.running(tool: "Read", toolUseId: "t2", subagent: true)) == "Sub-agent is running Read")
		#expect(AgentTurnStatus.label(.thinking(subagent: true)) == "Sub-agent is thinking")
		#expect(AgentTurnStatus.label(.writing(subagent: true)) == "Sub-agent is writing")
		#expect(AgentTurnStatus.label(.working) == "Working")
	}

	@Test func theTimerCountsWholeSecondsFromTheStepsStartAndNeverBelowZero() {
		let start: UInt64 = 1_800_000_000_000
		func at(_ milliseconds: Int64) -> Date { Date(timeIntervalSince1970: Double(Int64(start) + milliseconds) / 1000) }
		#expect(AgentTurnStatus.elapsed(since: start, at: at(999)) == "0s")
		#expect(AgentTurnStatus.elapsed(since: start, at: at(72_400)) == "1m 12s")
		#expect(AgentTurnStatus.elapsed(since: start, at: at(3_840_000)) == "1h 4m")
		#expect(AgentTurnStatus.elapsed(since: start, at: at(-500)) == "0s")
	}
}
