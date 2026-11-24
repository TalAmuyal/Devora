import Foundation
import CoreBindings
import TranscriptKit

/// The core's chat values as TranscriptKit's: the transcript has its own types, so that it builds and tests without the
/// core. They mirror the core's one to one, and a change to the core's makes this file stop compiling.

extension TranscriptKit.TranscriptItem {
	/// `name` names a model by its full ID (the core's rule, `ModelSuggestions.name(of:)`), which a model-change row shows
	/// instead of the IDs the core gives it.
	init(_ item: CoreBindings.TranscriptItem, name: (String) -> String?) {
		self.init(
			sequence: item.sequence, cliSession: item.cliSession, parentToolUseId: item.parentToolUseId,
			state: TranscriptKit.ItemState(item.state),
			content: Self.namingModels(TranscriptKit.TranscriptContent(item.content), name: name),
			createdAt: item.createdAtMs.map { Date(timeIntervalSince1970: Double($0) / 1000) })
	}

	/// A model-change row in the models' names, with its spoken form ("from … to …") in its detail as `spoken`. One the CLI
	/// worded (a fallback) keeps its sentence; a model with no name keeps its ID.
	private static func namingModels(
		_ content: TranscriptKit.TranscriptContent, name: (String) -> String?
	) -> TranscriptKit.TranscriptContent {
		guard case .system(.modelChanged, let text, let detailJson) = content,
			let detail = try? JSONSerialization.jsonObject(with: Data(detailJson.utf8)) as? [String: Any],
			let from = detail["from"] as? String, let to = detail["to"] as? String
		else { return content }
		let (fromName, toName) = (name(from) ?? from, name(to) ?? to)
		let fallback = detail["fallback"] as? Bool == true
		var named = detail
		named["spoken"] = fallback ? text : "Model changed from \(fromName) to \(toName)"
		let namedJson = (try? JSONSerialization.data(withJSONObject: named)).flatMap { String(data: $0, encoding: .utf8) }
		return .system(
			kind: .modelChanged, text: fallback ? text : "Model changed: \(fromName) → \(toName)",
			detailJson: namedJson ?? detailJson)
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
		case .assistant(let text, let model): self = .assistant(text: text, model: model)
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
		case .restartNotice: self = .restartNotice
		case .modelChanged: self = .modelChanged
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
