import AppKit
import CoreBindings
import Foundation
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
	/// A transcript in an offscreen window, laid out at 600 by 400 points.
	private func shown(_ items: [TranscriptItem], anchor: ChatScrollAnchor? = nil) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: anchor)
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
}

/// The scroll benchmark for the native transcript's target (no frame over 50 ms scrolling 5,000 rows), on a real window and display
/// link; it measures rather than asserts, and runs only with `SAPPHIRE_TRANSCRIPT_BENCH=1`, in a release build:
/// `SAPPHIRE_TRANSCRIPT_BENCH=1 swift test -c release -Xswiftc -enable-testing --filter TranscriptScrollBenchmark`
@MainActor
@Suite(.enabled(if: ProcessInfo.processInfo.environment["SAPPHIRE_TRANSCRIPT_BENCH"] == "1"))
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
