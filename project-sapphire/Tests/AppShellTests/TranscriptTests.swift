import AppKit
import CoreBindings
import Foundation
import SwiftUI
import Testing

@testable import AppShell

private func item(_ sequence: UInt64, _ content: TranscriptContent, state: ItemState = .complete) -> TranscriptItem {
	TranscriptItem(sequence: sequence, cliSession: "s", parentToolUseId: nil, state: state, content: content)
}

private func reply(_ sequence: UInt64, _ text: String = "A reply.") -> TranscriptItem {
	item(sequence, .assistant(text: text))
}

/// A transcript of `count` rows from `first`, mixing short and long replies so that rows differ in height.
private func transcript(_ count: Int, from first: UInt64 = 1) -> [TranscriptItem] {
	(0..<count).map { index in
		let sequence = first + UInt64(index)
		switch index % 3 {
		case 0: return item(sequence, .user(text: "Question \(sequence)", error: nil))
		case 1: return reply(sequence, String(repeating: "A longer reply that wraps over several lines. ", count: 12))
		default: return reply(sequence, "Short \(sequence).")
		}
	}
}

struct TranscriptChangeTests {
	@Test func olderHistoryIsAPrepend() {
		let rows = transcript(5, from: 10)
		#expect(TranscriptChange(from: rows, to: transcript(3, from: 7) + rows) == .prepended(count: 3))
	}

	@Test func streamingChangesOneRowInPlace() {
		let rows = transcript(4)
		var streamed = rows
		streamed[3] = reply(4, "More text")
		#expect(TranscriptChange(from: rows, to: streamed) == .updated(changed: [3], removedFromTop: 0, appended: 0, removedFromBottom: 0))
	}

	@Test func followingTheBottomTrimsTheTopWhileRowsAppend() {
		let rows = transcript(200)
		let next = Array(rows.dropFirst(2)) + transcript(2, from: 201)
		#expect(TranscriptChange(from: rows, to: next) == .updated(changed: [], removedFromTop: 2, appended: 2, removedFromBottom: 0))
	}

	@Test func anUnrelatedTranscriptReloads() {
		#expect(TranscriptChange(from: transcript(3), to: transcript(3, from: 50)) == .reloaded)
		#expect(TranscriptChange(from: [], to: transcript(3)) == .reloaded)
		#expect(TranscriptChange(from: transcript(3), to: transcript(3)) == .none)
	}
}

struct RowHeightsTests {
	@Test func aMeasuredRowIsExactUntilItsContentOrWidthChanges() {
		var heights = RowHeights()
		let row = reply(1)
		heights.record(row, width: 400, height: 50)
		#expect(heights.isExact(row, width: 400))
		#expect(heights.height(row, width: 400) == 50)
		#expect(!heights.isExact(reply(1, "Changed"), width: 400))
		#expect(!heights.isExact(row, width: 500))
		// At a new width, the old height is the estimate until it's measured again.
		#expect(heights.height(row, width: 500) == 50)
	}

	@Test func aChangedRowNeverEstimatesBelowItsLastHeight() {
		var heights = RowHeights()
		heights.record(reply(1, "Long text"), width: 400, height: 300)
		#expect(heights.height(reply(1, "Long text, streaming"), width: 400) == 300)
	}

	@Test func estimatesLearnFromMeasurements() {
		var heights = RowHeights()
		let measured = reply(1, String(repeating: "word ", count: 40))
		let guess = RowHeights.guess(measured, width: 400)
		heights.record(measured, width: 400, height: guess * 2)
		let other = reply(2, String(repeating: "word ", count: 40))
		#expect(heights.estimate(other, width: 400) == guess * 2)
		// Only that kind's estimates.
		let user = item(3, .user(text: "Hi", error: nil))
		#expect(heights.estimate(user, width: 400) == RowHeights.guess(user, width: 400))
	}

	@Test func keepForgetsRowsThatLeft() {
		var heights = RowHeights()
		heights.record(reply(1), width: 400, height: 50)
		heights.keep(only: [2])
		#expect(!heights.isExact(reply(1), width: 400))
	}
}

@MainActor
struct TranscriptScrollViewTests {
	/// A transcript in an offscreen window, laid out at 600 by 400 points, with no scroller: AppKit switches every scroll
	/// view to the system's preferred scroller style whenever it applies it again (a web view does, in another test), and
	/// a legacy scroller takes width, so a test's width would change under it.
	private func shown(_ items: [TranscriptItem], anchor: ChatScrollAnchor? = nil) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: anchor)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		view.autoresizingMask = [.width, .height]
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply(items)
		return (window, view)
	}

	/// Scrolls as the reader does (not as the view adjusts itself), putting `sequence`'s row `offset` below the top.
	private func readerScrolls(_ view: TranscriptScrollView, to sequence: UInt64, offset: CGFloat = 0) {
		let row = view.items.firstIndex { $0.sequence == sequence }!
		view.contentView.scroll(to: NSPoint(x: 0, y: view.table.rect(ofRow: row).minY - offset))
		view.reflectScrolledClipView(view.contentView)
	}

	/// Runs the idle measurement until every row is exact, yielding between frames as the display link would: the tests
	/// share the main actor, and holding it starves the others' polling.
	private func settle(_ view: TranscriptScrollView) async {
		var steps = 0
		while !view.isSettled && steps < 10_000 {
			view.measureIdle()
			steps += 1
			await Task.yield()
		}
	}

	@Test func itOpensAtTheBottomWithTheVisibleRowsMeasured() {
		let (window, view) = shown(transcript(300))
		defer { window.close() }
		#expect(view.distanceFromBottom == 0)
		// Opening measures only what is in view; the rest waits for idle frames.
		#expect(view.measurements < 40)
	}

	@Test func idleMeasurementMeasuresEveryRowOnceThenStops() async {
		let (window, view) = shown(transcript(300))
		defer { window.close() }
		await settle(view)
		#expect(view.isSettled)
		#expect(view.measurements == 300)
		#expect(view.distanceFromBottom == 0)
	}

	@Test func streamingAtTheBottomStaysAtTheBottom() {
		var rows = transcript(100)
		let (window, view) = shown(rows)
		defer { window.close() }
		for length in 1...20 {
			rows[99] = reply(100, String(repeating: "streamed words ", count: length * 10))
			view.apply(rows)
			#expect(view.distanceFromBottom == 0)
		}
	}

	@Test func theVisibleRowStaysWhileRowsAboveSettleAndBelowStream() async {
		var rows = transcript(300)
		let (window, view) = shown(rows)
		defer { window.close() }
		readerScrolls(view, to: 150, offset: 10)
		let anchor = view.visibleAnchor
		// The rows above are still estimates; measuring them changes their heights, not what is in view.
		await settle(view)
		#expect(view.visibleAnchor == anchor)
		rows[299] = reply(300, String(repeating: "streamed words ", count: 200))
		view.apply(rows)
		#expect(view.visibleAnchor == anchor)
	}

	@Test func scrollingUpThroughEstimatedRowsMovesTheSeenRowsByTheScrollOnly() async {
		let (window, view) = shown(transcript(300))
		defer { window.close() }
		readerScrolls(view, to: 150)
		var stepsMeasuring = 0
		for step in 0..<20 {
			let top = view.contentView.bounds.minY
			let visible = view.table.rows(in: view.contentView.bounds)
			let seen = (visible.location..<(visible.location + visible.length)).map { (view.items[$0].sequence, view.table.rect(ofRow: $0).minY - top) }
			let measured = view.measurements
			view.contentView.scroll(to: NSPoint(x: 0, y: top - 150))
			view.reflectScrolledClipView(view.contentView)
			if view.measurements > measured { stepsMeasuring += 1 }
			let newTop = view.contentView.bounds.minY
			for (sequence, offset) in seen {
				let row = view.items.firstIndex { $0.sequence == sequence }!
				#expect(view.table.rect(ofRow: row).minY - newTop == offset + 150, "Row \(sequence) moved at step \(step)")
			}
			await Task.yield()
		}
		#expect(stepsMeasuring >= 15, "The steps must bring estimated rows into view")
	}

	@Test func olderHistoryGoesInAboveWithoutMovingTheVisibleRow() {
		let rows = transcript(200, from: 201)
		let (window, view) = shown(rows)
		defer { window.close() }
		readerScrolls(view, to: 205)
		let anchor = view.visibleAnchor
		view.apply(transcript(200, from: 1) + rows)
		#expect(view.visibleAnchor == anchor)
	}

	@Test func aSavedPositionIsRestoredOnOpening() async {
		let (window, view) = shown(transcript(300), anchor: ChatScrollAnchor(sequence: 120, offset: -5))
		defer { window.close() }
		#expect(view.visibleAnchor == ChatScrollAnchor(sequence: 120, offset: -5))
		await settle(view)
		#expect(view.visibleAnchor == ChatScrollAnchor(sequence: 120, offset: -5))
	}

	@Test func aSavedPositionWhoseRowIsGoneOpensAtTheBottom() {
		let (window, view) = shown(transcript(300, from: 1000), anchor: ChatScrollAnchor(sequence: 5, offset: 0))
		defer { window.close() }
		#expect(view.distanceFromBottom == 0)
	}

	@Test func scrollingThroughThousandsOfRowsMeasuresOnlyTheRowsComingIntoView() async {
		let (window, view) = shown(transcript(2000))
		defer { window.close() }
		var worst = 0
		var top = view.contentView.bounds.minY
		while top > 0 {
			let before = view.measurements
			top = max(0, top - 400)
			view.contentView.scroll(to: NSPoint(x: 0, y: top))
			view.reflectScrolledClipView(view.contentView)
			worst = max(worst, view.measurements - before)
			await Task.yield()
		}
		// A screen's worth of new rows at a time, never a batch of hundreds.
		#expect(worst < 30)
	}

	@Test func aNewWidthKeepsTheVisibleRowAndMeasuresItAgain() async {
		let (window, view) = shown(transcript(300))
		defer { window.close() }
		await settle(view)
		readerScrolls(view, to: 150)
		let anchor = view.visibleAnchor
		window.setContentSize(NSSize(width: 400, height: 400))
		window.layoutIfNeeded()
		#expect(view.visibleAnchor == anchor)
		#expect(!view.isSettled)
		await settle(view)
		#expect(view.visibleAnchor == anchor)
	}

	/// The batches of one long turn past the chat's 200-row tail: the prompt, then replies, then more replies that push
	/// the oldest rows out of the tail.
	private func longTurn(_ view: TranscriptScrollView) {
		let prompt = item(1, .user(text: "history:240", error: nil))
		let replies = (0..<241).map { reply(UInt64($0 + 2), "History turn \($0)") }
		view.apply([prompt])
		view.apply([prompt] + replies.prefix(120))
		view.apply(Array(([prompt] + replies).suffix(200)))
	}

	/// The heights the table asked the transcript for, by row; nil for a row it never asked about.
	private func heightsGiven(_ view: TranscriptScrollView) -> [CGFloat?] {
		view.entries.map { view.tableHeights[$0.key] }
	}

	@Test func aBatchThatPushesRowsOutOfTheTailLaysOutEveryRowAtItsHeight() {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		defer { window.close() }
		let view = TranscriptScrollView(anchor: nil)
		view.frame = window.contentView!.bounds
		view.autoresizingMask = [.width, .height]
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		longTurn(view)
		// Read before anything asks for the rows' rects, which is what makes the table take their heights: a table that
		// estimated some rows itself would move them later, outside `settle`.
		let laidOut = view.table.rect(ofRow: view.table.numberOfRows - 1).maxY
		let given = heightsGiven(view)
		#expect(given.allSatisfy { $0 != nil })
		#expect(laidOut == given.reduce(0) { $0 + ($1 ?? 0) })
	}

	@Test func settledRowsAreLaidOutAtTheirMeasuredHeights() async {
		let (window, view) = shown([])
		defer { window.close() }
		longTurn(view)
		await settle(view)
		// A row the table took at an estimate that changed since (the calibration learns from every measurement) must
		// still be told its measured height.
		let wrong = (0..<view.table.numberOfRows).filter { view.table.rect(ofRow: $0).height != view.tableView(view.table, heightOfRow: $0) }
		#expect(wrong.isEmpty)
	}
}

/// The scroll benchmark for the native transcript's target (no frame over 50 ms scrolling 5,000 rows), on a real window and display
/// link; it measures rather than asserts, and runs only with `SAPPHIRE_TRANSCRIPT_BENCH=1`, in a release build:
/// `SAPPHIRE_TRANSCRIPT_BENCH=1 swift test -c release -Xswiftc -enable-testing --filter TranscriptScrollBenchmark`
@MainActor
// Serialized: the benchmarks share the main actor, and one running beside another measures both.
@Suite(.serialized, .enabled(if: ProcessInfo.processInfo.environment["SAPPHIRE_TRANSCRIPT_BENCH"] == "1"))
struct TranscriptScrollBenchmark {
	@Test func scrollingUpThroughFiveThousandRows() async {
		NSApplication.shared.setActivationPolicy(.accessory)
		let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 900, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: nil)
		view.frame = window.contentView!.bounds
		view.autoresizingMask = [.width, .height]
		window.contentView!.addSubview(view)
		window.orderFrontRegardless()
		defer { window.close() }
		let opened = CACurrentMediaTime()
		view.apply(transcript(5000))
		let openMs = (CACurrentMediaTime() - opened) * 1000
		let driver = FrameDriver(view: view, frames: 600) { view.contentView.scroll(to: NSPoint(x: 0, y: max(0, view.contentView.bounds.minY - 40))); view.reflectScrolledClipView(view.contentView) }
		let times = await driver.run()
		let intervals = zip(times.dropFirst(), times).map { ($0 - $1) * 1000 }
		let nominal = intervals.sorted()[intervals.count / 2]
		let settleStart = CACurrentMediaTime()
		let idle = FrameDriver(view: view, frames: 10_000, until: { view.isSettled }) {}
		_ = await idle.run()
		print("""
			TRANSCRIPT BENCH: open \(Int(openMs)) ms; \(intervals.count) frames at \(String(format: "%.1f", nominal)) ms; \
			longest \(String(format: "%.1f", intervals.max() ?? 0)) ms; over 1.5x: \(intervals.filter { $0 > nominal * 1.5 }.count); \
			over 50 ms: \(intervals.filter { $0 > 50 }.count); measured \(view.measurements); \
			settled \(view.isSettled) after \(Int((CACurrentMediaTime() - settleStart) * 1000)) ms more
			""")
	}

	/// The tentative goals at the chat's 200-row tail: opening, and a streaming update, each until drawn (the update,
	/// the layout and the window's display, in process; the compositor's frame comes after).
	@Test func openingAndStreamingTwoHundredRows() async {
		NSApplication.shared.setActivationPolicy(.accessory)
		var opens: [Double] = []
		for _ in 0..<10 {
			let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 900, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
			window.isReleasedWhenClosed = false
			let view = TranscriptScrollView(anchor: nil)
			view.frame = window.contentView!.bounds
			view.autoresizingMask = [.width, .height]
			window.contentView!.addSubview(view)
			window.orderFrontRegardless()
			window.displayIfNeeded()
			let start = CACurrentMediaTime()
			view.apply(transcript(200))
			window.displayIfNeeded()
			opens.append((CACurrentMediaTime() - start) * 1000)
			window.close()
			await Task.yield()
		}
		let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 900, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: nil)
		view.frame = window.contentView!.bounds
		view.autoresizingMask = [.width, .height]
		window.contentView!.addSubview(view)
		window.orderFrontRegardless()
		defer { window.close() }
		var rows = transcript(200)
		view.apply(rows)
		window.displayIfNeeded()
		var updates: [Double] = []
		var words = ""
		let driver = FrameDriver(view: view, frames: 120) {
			words += "streamed words, more of them "
			rows[199] = reply(200, "A streaming **reply**: " + words)
			let start = CACurrentMediaTime()
			view.apply(rows)
			window.displayIfNeeded()
			updates.append((CACurrentMediaTime() - start) * 1000)
		}
		_ = await driver.run()
		func summary(_ values: [Double]) -> String {
			let sorted = values.sorted()
			return String(format: "median %.1f ms, p95 %.1f ms, max %.1f ms", sorted[sorted.count / 2], sorted[Int(Double(sorted.count) * 0.95)], sorted.last ?? 0)
		}
		print("TRANSCRIPT BENCH 200: open \(summary(opens)); streaming update \(summary(updates))")
	}
}

/// Runs `step` on each display frame of `view`'s screen, `frames` times or until `until`, and returns the frames' times.
@MainActor
private final class FrameDriver: NSObject {
	private let frames: Int
	private let until: () -> Bool
	private let step: () -> Void
	private let view: NSView
	private var times: [Double] = []
	private var done: CheckedContinuation<[Double], Never>?

	init(view: NSView, frames: Int, until: @escaping () -> Bool = { false }, step: @escaping () -> Void) {
		self.view = view
		self.frames = frames
		self.until = until
		self.step = step
	}

	func run() async -> [Double] {
		await withCheckedContinuation { continuation in
			done = continuation
			view.displayLink(target: self, selector: #selector(tick)).add(to: .main, forMode: .common)
		}
	}

	@objc private func tick(_ link: CADisplayLink) {
		times.append(link.timestamp)
		step()
		if times.count >= frames || until() {
			link.invalidate()
			done?.resume(returning: times)
			done = nil
		}
	}
}

struct TranscriptLinkTests {
	@Test func onlyWebLinksOpen() {
		for link in ["https://example.com/a", "http://example.com", "HTTPS://EXAMPLE.COM"] {
			#expect(TranscriptRow.opensLink(URL(string: link)!), "\(link) must open")
		}
		let refused = [
			"file:///Applications/Calculator.app", "x-apple.systempreferences:com.apple.preference.security",
			"vscode://file/etc/passwd", "mailto:someone@example.com", "javascript:alert(1)", "relative/path",
		]
		for link in refused {
			#expect(!TranscriptRow.opensLink(URL(string: link)!), "\(link) must not open")
		}
	}
}

struct WheelRoutingTests {
	private typealias Wheel = (phase: NSEvent.Phase, momentum: NSEvent.Phase, dx: CGFloat, dy: CGFloat)

	/// Whether each event of a gesture goes to the transcript.
	private func routes(_ events: [Wheel]) -> [Bool] {
		var routing = WheelRouting()
		return events.map { routing.toTranscript(phase: $0.phase, momentumPhase: $0.momentum, deltaX: $0.dx, deltaY: $0.dy) }
	}

	@Test func aVerticalSwipeScrollsTheTranscriptThroughItsMomentum() {
		let swipe: [Wheel] = [
			(.mayBegin, [], 0, 0), (.began, [], 1, -4),
			(.changed, [], 6, -1),  // drifting sideways mid-gesture
			(.ended, [], 0, 0), ([], .began, 3, -1), ([], .ended, 0, 0),
		]
		#expect(routes(swipe) == [false, true, true, true, true, true])
	}

	@Test func aHorizontalSwipeStaysWithWhatIsUnderThePointerAndTheNextGestureDecidesAgain() {
		let swipes: [Wheel] = [
			(.began, [], -5, 1), (.changed, [], -1, 4), ([], .changed, -2, 0),
			(.mayBegin, [], 0, 0), (.began, [], 0, 3),
		]
		#expect(routes(swipes) == [false, false, false, false, true])
	}

	@Test func aMouseWheelTickIsDecidedOnItsOwn() {
		// Shift with a mouse wheel scrolls sideways.
		#expect(routes([([], [], 0, -10), ([], [], -10, 0)]) == [true, false])
	}
}

/// Which transcript the wheel monitor gives a scroll to, in SwiftUI windows laid out as the app's: the tabs in a `ZStack`,
/// with only the active one visible and hit-testable (`TabContent`), and window layers in an overlay over them.
@MainActor
struct TranscriptWheelTargetTests {
	private final class Views {
		var byName: [String: TranscriptScrollView] = [:]
	}

	private func view(_ name: String, into views: Views) -> TranscriptView {
		TranscriptView(items: transcript(50), hasOlder: false, anchor: nil, onScroll: { _ in }, onNeedsOlder: {}, onView: { views.byName[name] = $0 })
	}

	private func shown(_ content: some View) async -> NSWindow {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		window.contentView = NSHostingView(rootView: content.frame(width: 600, height: 400))
		for _ in 0..<5 {
			window.layoutIfNeeded()
			await Task.yield()
		}
		return window
	}

	@Test func onlyTheActiveTabsTranscriptTakesTheWheel() async throws {
		let views = Views()
		let window = await shown(ZStack {
			ForEach(["hidden", "active"], id: \.self) { name in
				view(name, into: views).opacity(name == "active" ? 1 : 0).allowsHitTesting(name == "active")
			}
		})
		defer { window.close() }
		let middle = NSPoint(x: 300, y: 200)
		#expect(try #require(views.byName["active"]).isUnderPointer(at: middle))
		#expect(try !#require(views.byName["hidden"]).isUnderPointer(at: middle))
	}

	@Test func aCodeBlocksOwnScrollViewIsPartOfTheTranscript() async throws {
		let views = Views()
		let lines = (0..<30).map { "line \($0): " + String(repeating: "wide ", count: 80) }.joined(separator: "\n")
		let code = TranscriptItem(sequence: 1, cliSession: "s", parentToolUseId: nil, state: .complete, content: .assistant(text: "```\n\(lines)\n```"))
		let window = await shown(TranscriptView(items: [code], hasOlder: false, anchor: nil, onScroll: { _ in }, onNeedsOlder: {}, onView: { views.byName["code"] = $0 }))
		defer { window.close() }
		let view = try #require(views.byName["code"])
		let point = NSPoint(x: 300, y: 200)
		let hit = try #require(window.contentView?.hitTest(window.contentView!.superview!.convert(point, from: nil)))
		#expect(hit !== view && hit !== view.table, "The point must be over a row's content, not the bare table")
		#expect(view.isUnderPointer(at: point))
	}

	@Test func aLayerOverTheTabKeepsTheWheel() async throws {
		let views = Views()
		let window = await shown(view("tab", into: views).overlay(alignment: .leading) {
			Color.gray.frame(width: 300)
		})
		defer { window.close() }
		let view = try #require(views.byName["tab"])
		#expect(!view.isUnderPointer(at: NSPoint(x: 150, y: 200)), "Under the layer")
		#expect(view.isUnderPointer(at: NSPoint(x: 450, y: 200)), "Beside the layer")
	}
}

private func thought(_ sequence: UInt64, _ text: String = "Consider **this**.") -> TranscriptItem {
	item(sequence, .thinking(text: text, signature: "s"))
}

private func tool(_ sequence: UInt64, verdict: String? = "approve", result: String? = "/tmp") -> TranscriptItem {
	item(sequence, .tool(
		toolUseId: "t\(sequence)", name: "Bash", inputJson: #"{"description":"cwd","command":"pwd"}"#,
		verdict: verdict.map { ToolVerdict(kind: $0, rule: nil, detail: "Approve(rule)") },
		result: result.map { ToolResult(text: $0, contentJson: "", isError: false) }))
}

struct TranscriptRowModelTests {
	@Test func copyTakesTheSourceAndTheBridgeTheWebTranscriptsText() {
		let reply = TranscriptRowModel(item(1, .assistant(text: "A **bold** [link](https://example.com)")), document: nil)
		#expect(reply.source == "A **bold** [link](https://example.com)")
		let call = TranscriptRowModel(tool(2), document: nil)
		#expect(call.source == "Bash\n\n{\n  \"command\" : \"pwd\",\n  \"description\" : \"cwd\"\n}\n\n/tmp")
		#expect(call.plainText.hasPrefix("Bash\nJudgess approve\n{"))
		#expect(TranscriptRowModel(tool(3, verdict: nil, result: nil), document: nil).plainText.hasSuffix("Awaiting result…"))
	}

	@Test func voiceOverHearsTheKindThenWhatIsShown() {
		let collapsed = TranscriptRowModel(tool(1), document: nil)
		#expect(collapsed.accessibilityLabel == "Tool Bash, Judgess approve, collapsed")
		#expect(collapsed.accessibilityValue == "")
		let open = TranscriptRowModel(thought(2), document: nil, expanded: true)
		#expect(open.accessibilityLabel == "Thinking, expanded")
		#expect(open.accessibilityValue == "Consider **this**.")
		let stopped = TranscriptRowModel(item(3, .assistant(text: "Half"), state: .interrupted), document: nil)
		#expect(stopped.accessibilityLabel == "Agent, interrupted")
	}

	@Test func aDocumentListsItsLinksInReadingOrder() {
		let document = MarkdownDocument("See [one](https://a.example) and\n\n- [two](https://b.example)")
		#expect(document.links.map(\.text) == ["one", "two"])
	}
}

struct RowHeightsExpansionTests {
	@Test func expandedAndCollapsedMeasurementsNeverMix() {
		var heights = RowHeights()
		heights.record(thought(1), expanded: false, width: 400, height: 28)
		#expect(heights.isExact(thought(1), expanded: false, width: 400))
		#expect(!heights.isExact(thought(1), expanded: true, width: 400))
		#expect(heights.height(thought(1), expanded: true, width: 400) != 28)
		heights.record(thought(1), expanded: true, width: 400, height: 90)
		#expect(heights.height(thought(1), expanded: true, width: 400) == 90)
		#expect(!heights.isExact(thought(1), expanded: false, width: 400))
	}
}

@MainActor
struct TranscriptExpansionTests {
	private func shown(_ items: [TranscriptItem]) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: nil)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply(items)
		return (window, view)
	}

	@Test func rowsStartCollapsedAndOpenInPlace() {
		var rows = transcript(40)
		rows.insert(tool(1000), at: 30)
		rows.insert(thought(1001), at: 31)
		let (window, view) = shown(rows)
		defer { window.close() }
		#expect(!view.isFirstExpanded(.tool) && !view.isFirstExpanded(.thinking))
		// Scrolled up, so that the tool row is in view, not at the bottom.
		let row = view.items.firstIndex { $0.sequence == 1000 }!
		view.contentView.scroll(to: NSPoint(x: 0, y: view.table.rect(ofRow: row).minY - 40))
		view.reflectScrolledClipView(view.contentView)
		let collapsed = view.table.rect(ofRow: row)
		let offset = collapsed.minY - view.contentView.bounds.minY
		view.toggle(1000)
		#expect(view.isFirstExpanded(.tool))
		#expect(view.table.rect(ofRow: row).height > collapsed.height, "Its input and result open below the summary")
		#expect(view.table.rect(ofRow: row).minY - view.contentView.bounds.minY == offset, "Its top stays where it was")
		view.toggle(1000)
		#expect(view.table.rect(ofRow: row).height == collapsed.height)
	}

	@Test func anExpandedRowStaysExpandedThroughUpdates() {
		var rows = [thought(1), tool(2, result: nil)] + transcript(5, from: 10)
		let (window, view) = shown(rows)
		defer { window.close() }
		view.toggle(2)
		rows[1] = tool(2, result: "done")
		rows.append(reply(20, "More"))
		view.apply(rows)
		#expect(view.isFirstExpanded(.tool))
		#expect(!view.isFirstExpanded(.thinking))
	}

	@Test func theBridgeSeesKindsExpansionAndTheVerdict() throws {
		let (window, view) = shown([item(1, .user(text: "Hi", error: nil)), thought(2), tool(3)])
		defer { window.close() }
		view.toggleFirst(.thinking)
		let observation = view.observation()
		let rows = try #require(observation["rows"] as? [[String: Any]])
		#expect(rows.compactMap { $0["kind"] as? String } == ["chat-item chat-user", "chat-item chat-thinking", "chat-item chat-tool"])
		#expect(observation["thinkingOpen"] as? Bool == true)
		#expect(observation["toolOpen"] as? Bool == false)
		#expect(observation["verdict"] as? String == "Judgess approve")
	}

	@Test func aCellWaitingForReuseShowsNothing() {
		let (window, view) = shown(transcript(10))
		defer { window.close() }
		let rowView = view.table.rowView(atRow: 9, makeIfNecessary: true)!
		let cell = rowView.view(atColumn: 0) as! TranscriptCell
		#expect(cell.isShowing)
		view.tableView(view.table, didRemove: rowView, forRow: -1)
		#expect(!cell.isShowing)
	}
}

@MainActor
struct TranscriptToggleAtTheBottomTests {
	@Test func aRowOpenedAtTheBottomStaysWhereItWasClicked() {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		defer { window.close() }
		let view = TranscriptScrollView(anchor: nil)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply(transcript(40) + [tool(1000, result: String(repeating: "line\n", count: 40)), reply(1001, "Done.")])
		#expect(view.distanceFromBottom == 0)
		let row = view.items.firstIndex { $0.sequence == 1000 }!
		let offset = view.table.rect(ofRow: row).minY - view.contentView.bounds.minY
		view.toggle(1000)
		#expect(view.table.rect(ofRow: row).minY - view.contentView.bounds.minY == offset)
		#expect(view.distanceFromBottom > 0, "The opened body pushes the bottom out of view, and the view stays")
	}
}
