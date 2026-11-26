import Foundation

/// What the transcript shows, in TranscriptKit's own types, so that it builds and tests without the Rust core.
/// They mirror the core's chat projection (`devora-agent::transcript`), and the host maps the core's values into them;
/// the names, cases and labels are the core's, so the mapping is one to one.

/// One item of the conversation, by its stable sequence number.
public struct TranscriptItem: Equatable, Hashable, Sendable {
	public var sequence: UInt64
	public var cliSession: String
	public var parentToolUseId: String?
	public var state: ItemState
	public var content: TranscriptContent
	/// When the item was first stored; none for an item stored before Devora kept the time.
	public var createdAt: Date?

	public init(
		sequence: UInt64, cliSession: String, parentToolUseId: String?, state: ItemState, content: TranscriptContent,
		createdAt: Date?
	) {
		self.sequence = sequence
		self.cliSession = cliSession
		self.parentToolUseId = parentToolUseId
		self.state = state
		self.content = content
		self.createdAt = createdAt
	}
}

public enum ItemState: Equatable, Hashable, Sendable {
	case pending
	case streaming
	case complete
	case interrupted
	case failed
}

public enum TranscriptContent: Equatable, Hashable, Sendable {
	/// `delivery` is where a prompt the agent doesn't have yet is; none once it has it.
	case user(text: String, error: String?, delivery: Delivery?)
	/// `model` is the model that wrote it, as the API names it; none for a reply stored before it was kept.
	case assistant(text: String, model: String?)
	case thinking(text: String, signature: String)
	case tool(toolUseId: String, name: String, inputJson: String, verdict: ToolVerdict?, result: ToolResult?)
	case system(kind: SystemKind, text: String, detailJson: String)
	case unknown(json: String)
}

/// A prompt sent into a turn the agent started itself, which the agent doesn't have yet.
public enum Delivery: Equatable, Hashable, Sendable {
	/// It waits for the agent's next step.
	case waiting
	/// Its session ended first: the agent never got it.
	case notDelivered

	/// What the row says of it.
	public var note: String {
		switch self {
		case .waiting: "Not yet received"
		case .notDelivered: "Not delivered: send it again"
		}
	}
}

/// Judgess's verdict on a tool call.
public struct ToolVerdict: Equatable, Hashable, Sendable {
	public var kind: String
	public var rule: String?
	public var detail: String

	public init(kind: String, rule: String?, detail: String) {
		self.kind = kind
		self.rule = rule
		self.detail = detail
	}
}

public struct ToolResult: Equatable, Hashable, Sendable {
	public var text: String
	public var contentJson: String
	public var isError: Bool

	public init(text: String, contentJson: String, isError: Bool) {
		self.text = text
		self.contentJson = contentJson
		self.isError = isError
	}
}

public enum SystemKind: Equatable, Hashable, Sendable {
	case resume
	case compaction
	case interruption
	case exit
	case failure
	case warning
	case permissionRequested
	case permissionAnswer
	case permissionCancelled
	case judgement
	case result
	case unfinishedFirstPrompt
	/// Devora restarted the agent's session and asked it to re-check its waits.
	case restartNotice
	/// The agent's replies now come from another model.
	case modelChanged
}

/// A tool call waiting for the user's answer, shown as a card.
public struct PermissionQuestion: Equatable, Hashable, Sendable {
	public var requestId: String
	public var toolName: String
	public var toolUseId: String
	public var inputJson: String
	public var decisionReason: String?
	public var canAlwaysAllow: Bool
	public var suggestedRulesJson: String?

	public init(
		requestId: String, toolName: String, toolUseId: String, inputJson: String, decisionReason: String?, canAlwaysAllow: Bool,
		suggestedRulesJson: String?
	) {
		self.requestId = requestId
		self.toolName = toolName
		self.toolUseId = toolUseId
		self.inputJson = inputJson
		self.decisionReason = decisionReason
		self.canAlwaysAllow = canAlwaysAllow
		self.suggestedRulesJson = suggestedRulesJson
	}
}

/// Where the reader is: an item's sequence and its top's offset from the visible area's top.
public struct ChatScrollAnchor: Equatable, Hashable, Sendable {
	public var sequence: UInt64
	public var offset: Int32

	public init(sequence: UInt64, offset: Int32) {
		self.sequence = sequence
		self.offset = offset
	}
}
