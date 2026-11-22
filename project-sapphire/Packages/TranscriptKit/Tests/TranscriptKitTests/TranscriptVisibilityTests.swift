import AppKit
import Testing

@testable import TranscriptKit

private func item(_ sequence: UInt64, _ content: TranscriptContent, state: ItemState = .complete) -> TranscriptItem {
	TranscriptItem(sequence: sequence, cliSession: "s", parentToolUseId: nil, state: state, content: content)
}

private func system(_ kind: SystemKind, detail: String = "{}") -> TranscriptContent {
	.system(kind: kind, text: "", detailJson: detail)
}

private let turnCompleted = system(.result, detail: #"{"is_error":false}"#)
private let turnFailed = system(.result, detail: #"{"is_error":true}"#)
private let turnStopped = system(.result, detail: #"{"is_error":true,"stopped":true}"#)

@MainActor
struct TranscriptVisibilityTests {
	@Test func theConversationAndWhatNeedsTheUserAlwaysShowTheAgentsWorkingIsSystemAndAPlainTurnEndNever() {
		let tool = TranscriptContent.tool(toolUseId: "t1", name: "Bash", inputJson: "{}", verdict: nil, result: nil)
		let cases: [(TranscriptItem, TranscriptVisibility)] = [
			(item(1, .user(text: "Hi", error: nil)), .always),
			(item(2, .assistant(text: "Hello", model: nil)), .always),
			(item(3, system(.modelChanged)), .always),
			(item(4, system(.unfinishedFirstPrompt)), .always),
			(item(5, system(.failure)), .always),
			(item(6, system(.warning)), .always),
			(item(7, turnFailed), .always),
			(item(8, tool, state: .failed), .always),
			(item(9, tool), .system),
			(item(10, .thinking(text: "Hm", signature: "")), .system),
			(item(11, .unknown(json: "{}")), .system),
			(item(12, system(.judgement)), .system),
			(item(13, system(.permissionCancelled)), .system),
			(item(14, system(.compaction)), .system),
			(item(15, system(.restartNotice)), .system),
			(item(16, system(.interruption)), .system),
			(item(17, turnCompleted), .never),
			(item(18, turnStopped), .system),
		]
		for (item, visibility) in cases {
			#expect(item.visibility == visibility, "#\(item.sequence) \(item.content)")
		}
	}

	@Test func aRowNeverShownIsNeitherLaidOutNorMeasured() async {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		defer { window.close() }
		let view = TranscriptScrollView(anchor: nil)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply([
			item(1, .user(text: "Hi", error: nil)), item(2, .assistant(text: "Hello", model: nil)), item(3, turnCompleted),
			item(4, .user(text: "Again", error: nil)), item(5, turnFailed),
		])
		var steps = 0
		while !view.isSettled && steps < 1000 {
			view.measureIdle()
			steps += 1
			await Task.yield()
		}
		#expect(view.items.map(\.sequence) == [1, 2, 4, 5])
		#expect(view.table.numberOfRows == 4)
		#expect(view.measurements == 4)
	}
}
