@testable import AppShell
import Foundation
import CoreBindings
import Testing
import TranscriptKit

/// The core's chat values reach the transcript field for field: the types mirror each other, and a swapped field would
/// still compile.
struct TranscriptAdapterTests {
	private func item(_ content: CoreBindings.TranscriptContent, state: CoreBindings.ItemState = .complete) -> CoreBindings.TranscriptItem {
		CoreBindings.TranscriptItem(
			sequence: 7, cliSession: "cli", parentToolUseId: "parent", state: state, content: content,
			createdAtMs: 1_795_000_000_123)
	}

	@Test func everyKindOfItemKeepsItsFields() {
		let verdict = CoreBindings.ToolVerdict(kind: "approve", rule: "Bash(ls)", detail: "safe")
		let result = CoreBindings.ToolResult(text: "out", contentJson: "{}", isError: true)
		let cases: [(CoreBindings.TranscriptContent, TranscriptKit.TranscriptContent)] = [
			(.user(text: "hi", error: "oops", delivery: nil), .user(text: "hi", error: "oops", delivery: nil)),
			(.user(text: "hi", error: nil, delivery: .waiting), .user(text: "hi", error: nil, delivery: .waiting)),
			(.user(text: "hi", error: nil, delivery: .notDelivered), .user(text: "hi", error: nil, delivery: .notDelivered)),
			(.assistant(text: "hello", model: "claude-opus-5-5"), .assistant(text: "hello", model: "claude-opus-5-5")),
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
				TranscriptKit.TranscriptItem(item(core, state: .streaming), name: { _ in nil })
					== TranscriptKit.TranscriptItem(
						sequence: 7, cliSession: "cli", parentToolUseId: "parent", state: .streaming, content: expected,
						createdAt: Date(timeIntervalSince1970: 1_795_000_000.123)))
		}
		var undated = item(.unknown(json: "{}"))
		undated.createdAtMs = nil
		#expect(TranscriptKit.TranscriptItem(undated, name: { _ in nil }).createdAt == nil)
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
