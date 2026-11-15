@testable import AppShell
import CoreBindings
import Testing
import TranscriptKit

/// The core's chat values reach the transcript field for field: the types mirror each other, and a swapped field would
/// still compile.
struct TranscriptAdapterTests {
	private func item(_ content: CoreBindings.TranscriptContent, state: CoreBindings.ItemState = .complete) -> CoreBindings.TranscriptItem {
		CoreBindings.TranscriptItem(sequence: 7, cliSession: "cli", parentToolUseId: "parent", state: state, content: content)
	}

	@Test func everyKindOfItemKeepsItsFields() {
		let verdict = CoreBindings.ToolVerdict(kind: "approve", rule: "Bash(ls)", detail: "safe")
		let result = CoreBindings.ToolResult(text: "out", contentJson: "{}", isError: true)
		let cases: [(CoreBindings.TranscriptContent, TranscriptKit.TranscriptContent)] = [
			(.user(text: "hi", error: "oops"), .user(text: "hi", error: "oops")),
			(.assistant(text: "hello"), .assistant(text: "hello")),
			(.thinking(text: "hmm", signature: "sig"), .thinking(text: "hmm", signature: "sig")),
			(
				.tool(toolUseId: "t1", name: "Bash", inputJson: "{\"command\":\"ls\"}", verdict: verdict, result: result),
				.tool(
					toolUseId: "t1", name: "Bash", inputJson: "{\"command\":\"ls\"}",
					verdict: TranscriptKit.ToolVerdict(kind: "approve", rule: "Bash(ls)", detail: "safe"),
					result: TranscriptKit.ToolResult(text: "out", contentJson: "{}", isError: true))
			),
			(
				.system(kind: .permissionAnswer, text: "allowed", detailJson: "{\"a\":1}"),
				.system(kind: .permissionAnswer, text: "allowed", detailJson: "{\"a\":1}")
			),
			(.unknown(json: "{}"), .unknown(json: "{}")),
		]
		for (core, expected) in cases {
			#expect(
				TranscriptKit.TranscriptItem(item(core, state: .streaming))
					== TranscriptKit.TranscriptItem(sequence: 7, cliSession: "cli", parentToolUseId: "parent", state: .streaming, content: expected))
		}
	}

	@Test func questionsAndTheScrollAnchorKeepTheirFields() {
		let question = CoreBindings.PermissionQuestion(
			requestId: "r1", toolName: "Bash", toolUseId: "t1", inputJson: "{}", decisionReason: "asks", canAlwaysAllow: true,
			suggestedRulesJson: "[]")
		#expect(
			TranscriptKit.PermissionQuestion(question)
				== TranscriptKit.PermissionQuestion(
					requestId: "r1", toolName: "Bash", toolUseId: "t1", inputJson: "{}", decisionReason: "asks", canAlwaysAllow: true,
					suggestedRulesJson: "[]"))
		let anchor = CoreBindings.ChatScrollAnchor(sequence: 42, offset: -13)
		#expect(CoreBindings.ChatScrollAnchor(TranscriptKit.ChatScrollAnchor(anchor)) == anchor)
	}
}
