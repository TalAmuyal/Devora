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

@MainActor
struct SystemMessagesToggleTests {
	/// A turn's worth of rows: the user's prompt, a tool call, and a reply long enough to wrap.
	private func turns(_ count: Int) -> [TranscriptItem] {
		(0..<UInt64(count)).flatMap { turn -> [TranscriptItem] in
			let first = turn * 3 + 1
			return [
				item(first, .user(text: "Question \(turn)", error: nil)),
				item(first + 1, .tool(toolUseId: "t\(turn)", name: "Bash", inputJson: "{}", verdict: nil, result: nil)),
				item(first + 2, .assistant(text: String(repeating: "A reply that wraps. ", count: 20), model: nil)),
			]
		}
	}

	private func shown(_ items: [TranscriptItem], showsSystem: Bool, anchor: ChatScrollAnchor? = nil) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: anchor)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.showsSystem = showsSystem
		view.apply(items)
		return (window, view)
	}

	/// The row's offset from the visible area's top.
	private func offset(of sequence: UInt64, in view: TranscriptScrollView) -> CGFloat {
		let row = view.items.firstIndex { $0.sequence == sequence }!
		return view.table.rect(ofRow: row).minY - view.contentView.bounds.minY
	}

	/// Scrolls as the reader does, putting `sequence`'s row at the top.
	private func readerScrolls(_ view: TranscriptScrollView, to sequence: UInt64) {
		let row = view.items.firstIndex { $0.sequence == sequence }!
		view.contentView.scroll(to: NSPoint(x: 0, y: view.table.rect(ofRow: row).minY))
		view.reflectScrolledClipView(view.contentView)
	}

	@Test func systemMessagesAreHiddenUnlessTheChatShowsThem() {
		let (window, view) = shown(turns(2), showsSystem: false)
		defer { window.close() }
		#expect(view.items.map(\.sequence) == [1, 3, 4, 6])
		view.showsSystem = true
		view.apply(turns(2))
		#expect(view.items.map(\.sequence) == [1, 2, 3, 4, 5, 6])
	}

	@Test func hidingKeepsTheReaderOnTheNearestShownRowAboveAndSavesIt() throws {
		let (window, view) = shown(turns(30), showsSystem: true)
		defer { window.close() }
		var saved: [ChatScrollAnchor?] = []
		view.onScroll = { saved.append($0) }
		// The reader is on a tool call, the system message the toggle hides.
		readerScrolls(view, to: 32)
		let prompt = offset(of: 31, in: view)
		view.showsSystem = false
		view.apply(turns(30))
		#expect(offset(of: 31, in: view) == prompt, "the prompt above the hidden row stays where it was")
		let top = try #require(view.visibleAnchor)
		#expect(top.sequence == 33, "the reply below it moves up into the hidden row's place")
		#expect(saved.last ?? nil == top, "and is saved as the reader's position, which names no hidden row")
		let reply = offset(of: 33, in: view)
		view.showsSystem = true
		view.apply(turns(30))
		#expect(offset(of: 33, in: view) == reply, "showing them again keeps the row on top where it is")
	}

	@Test func aToggleAtTheBottomStaysAtTheBottom() {
		let (window, view) = shown(turns(30), showsSystem: false)
		defer { window.close() }
		view.showsSystem = true
		view.apply(turns(30))
		#expect(view.distanceFromBottom == 0)
	}

	@Test func aSavedPositionOnAHiddenRowOpensAtTheNearestShownRowAbove() {
		let (window, view) = shown(turns(30), showsSystem: false, anchor: ChatScrollAnchor(sequence: 32, offset: 0))
		defer { window.close() }
		#expect(view.distanceFromBottom > 0, "not at the bottom")
		#expect(offset(of: 31, in: view) == 0)
	}
}
