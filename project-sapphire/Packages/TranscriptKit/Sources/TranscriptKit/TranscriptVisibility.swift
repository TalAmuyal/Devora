import Foundation

/// Which rows the chat shows: the conversation and what needs the user always, the agent's working (tools, thinking,
/// the session's own notes, a turn Devora stopped) as system messages, and a turn's plain end never, since the
/// spinner stopping says it.
enum TranscriptVisibility: Equatable {
	case always
	case system
	case never
}

extension TranscriptItem {
	var visibility: TranscriptVisibility {
		if isError { return .always }
		switch content {
		case .user, .assistant: return .always
		case .thinking, .tool, .unknown: return .system
		case .system(let kind, _, _):
			switch kind {
			// A model change says replies now come from another model; the marker, that a prompt didn't reach the agent.
			case .modelChanged, .unfinishedFirstPrompt, .failure, .warning: return .always
			case .result: return resultDetail?["stopped"] as? Bool == true ? .system : .never
			case .resume, .compaction, .interruption, .exit, .permissionRequested, .permissionAnswer, .permissionCancelled,
				.judgement, .restartNotice:
				return .system
			}
		}
	}

	/// What went wrong, shown whatever the system messages' toggle says: a failed item, a failure or a warning, and a
	/// turn that ended with an error, unless Devora stopped it (the core marks that result `stopped`: Claude Code reports
	/// an interrupted turn as an error).
	var isError: Bool {
		if state == .failed { return true }
		guard case .system(let kind, _, _) = content else { return false }
		switch kind {
		case .failure, .warning: return true
		case .result: return resultDetail?["is_error"] as? Bool == true && resultDetail?["stopped"] as? Bool != true
		default: return false
		}
	}

	/// A result row's detail: the CLI's result, with `stopped` when Devora stopped the turn.
	private var resultDetail: [String: Any]? {
		guard case .system(.result, _, let detailJson) = content else { return nil }
		return (try? JSONSerialization.jsonObject(with: Data(detailJson.utf8))) as? [String: Any]
	}
}
