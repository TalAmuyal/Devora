import CoreBindings
import TranscriptKit

/// The core's chat values as TranscriptKit's: the transcript has its own types, so that it builds and tests without the
/// core. They mirror the core's one to one, and a change to the core's makes this file stop compiling.

extension TranscriptKit.TranscriptItem {
	init(_ item: CoreBindings.TranscriptItem) {
		self.init(
			sequence: item.sequence, cliSession: item.cliSession, parentToolUseId: item.parentToolUseId,
			state: TranscriptKit.ItemState(item.state), content: TranscriptKit.TranscriptContent(item.content))
	}
}

extension TranscriptKit.ItemState {
	init(_ state: CoreBindings.ItemState) {
		switch state {
		case .pending: self = .pending
		case .streaming: self = .streaming
		case .complete: self = .complete
		case .interrupted: self = .interrupted
		case .failed: self = .failed
		}
	}
}

extension TranscriptKit.TranscriptContent {
	init(_ content: CoreBindings.TranscriptContent) {
		switch content {
		case .user(let text, let error): self = .user(text: text, error: error)
		case .assistant(let text): self = .assistant(text: text)
		case .thinking(let text, let signature): self = .thinking(text: text, signature: signature)
		case .tool(let toolUseId, let name, let inputJson, let verdict, let result):
			self = .tool(
				toolUseId: toolUseId, name: name, inputJson: inputJson, verdict: verdict.map(TranscriptKit.ToolVerdict.init),
				result: result.map(TranscriptKit.ToolResult.init))
		case .system(let kind, let text, let detailJson):
			self = .system(kind: TranscriptKit.SystemKind(kind), text: text, detailJson: detailJson)
		case .unknown(let json): self = .unknown(json: json)
		}
	}
}

extension TranscriptKit.ToolVerdict {
	init(_ verdict: CoreBindings.ToolVerdict) {
		self.init(kind: verdict.kind, rule: verdict.rule, detail: verdict.detail)
	}
}

extension TranscriptKit.ToolResult {
	init(_ result: CoreBindings.ToolResult) {
		self.init(text: result.text, contentJson: result.contentJson, isError: result.isError)
	}
}

extension TranscriptKit.SystemKind {
	init(_ kind: CoreBindings.SystemKind) {
		switch kind {
		case .resume: self = .resume
		case .compaction: self = .compaction
		case .interruption: self = .interruption
		case .exit: self = .exit
		case .failure: self = .failure
		case .warning: self = .warning
		case .permissionRequested: self = .permissionRequested
		case .permissionAnswer: self = .permissionAnswer
		case .permissionCancelled: self = .permissionCancelled
		case .judgement: self = .judgement
		case .result: self = .result
		case .unfinishedFirstPrompt: self = .unfinishedFirstPrompt
		}
	}
}

extension TranscriptKit.PermissionQuestion {
	init(_ question: CoreBindings.PermissionQuestion) {
		self.init(
			requestId: question.requestId, toolName: question.toolName, toolUseId: question.toolUseId,
			inputJson: question.inputJson, decisionReason: question.decisionReason, canAlwaysAllow: question.canAlwaysAllow,
			suggestedRulesJson: question.suggestedRulesJson)
	}
}

extension TranscriptKit.ChatScrollAnchor {
	init(_ anchor: CoreBindings.ChatScrollAnchor) {
		self.init(sequence: anchor.sequence, offset: anchor.offset)
	}
}

extension CoreBindings.ChatScrollAnchor {
	init(_ anchor: TranscriptKit.ChatScrollAnchor) {
		self.init(sequence: anchor.sequence, offset: anchor.offset)
	}
}
