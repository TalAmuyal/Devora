import AppKit
import CoreBindings
import Foundation
import Testing
import TestSupport

@testable import AppShell

/// A drawer that answers on the main actor's next turn, or once its `gate` opens; fails the sources it is told to,
/// never answers for the `hanging` ones, and records how many draws ran at once. Nothing in it takes time.
@MainActor
private final class FakeDrawer: DiagramDrawer {
	var gate: Gate?
	var failing: Set<String> = []
	var hanging: Set<String> = []
	private(set) var drawn: [String] = []
	private(set) var resets = 0
	private(set) var mostAtOnce = 0
	private(set) var atOnce = 0
	private let never = Gate()

	struct Failure: Error {}

	func draw(_ key: DiagramKey, theme: [String: String]) async throws -> NSImage {
		atOnce += 1
		mostAtOnce = max(mostAtOnce, atOnce)
		defer { atOnce -= 1 }
		if hanging.contains(key.source) { await never.wait() }
		if let gate { await gate.wait() } else { await Task.yield() }
		drawn.append(key.source)
		if failing.contains(key.source) { throw Failure() }
		return NSImage(size: CGSize(width: CGFloat(key.width), height: 100))
	}

	func reset() { resets += 1 }
}

/// What waits on it goes on once the test opens it.
@MainActor
private final class Gate {
	private var waiting: [CheckedContinuation<Void, Never>] = []
	private var isOpen = false

	func wait() async {
		guard !isOpen else { return }
		await withCheckedContinuation { waiting.append($0) }
	}

	func open() {
		isOpen = true
		let released = waiting
		waiting = []
		released.forEach { $0.resume() }
	}
}

/// The renderer's timeouts, in the test's hands: each draw's timeout waits until `fire()` lets the oldest one go, so no
/// draw times out on its own, however late it runs.
@MainActor
private final class TimeoutClock {
	private var waiting: [CheckedContinuation<Void, Never>] = []

	var sleep: @MainActor (Duration) async throws -> Void { { _ in await withCheckedContinuation { self.waiting.append($0) } } }

	/// The timeouts waiting to fire.
	var pending: Int { waiting.count }

	func fire() { waiting.removeFirst().resume() }
}

/// The diagrams `renderer` announces as done, which a test awaits rather than polling a deadline: observed from
/// before the test's first request, so none is missed.
@MainActor
private final class Announced {
	private(set) var keys: [DiagramKey] = []
	private var waiting: [(done: ([DiagramKey]) -> Bool, continuation: CheckedContinuation<Void, Never>)] = []

	init(_ renderer: MermaidRenderer) {
		_ = renderer.observe { [weak self] key in self?.add(key) }
	}

	/// Returns once `key` has been announced.
	func done(_ key: DiagramKey) async { await until { $0.contains(key) } }

	/// Returns once `count` diagrams have been announced.
	func count(_ count: Int) async { await until { $0.count >= count } }

	private func until(_ done: @escaping ([DiagramKey]) -> Bool) async {
		guard !done(keys) else { return }
		await withCheckedContinuation { waiting.append((done, $0)) }
	}

	private func add(_ key: DiagramKey) {
		keys.append(key)
		let (ready, rest) = (waiting.filter { $0.done(keys) }, waiting.filter { !$0.done(keys) })
		waiting = rest
		ready.forEach { $0.continuation.resume() }
	}
}

private func key(_ source: String, width: Int = 300, theme: [String: String] = ["--color-bg": "#000"]) -> DiagramKey {
	DiagramKey(source: source, width: width, scale: 2, theme: theme)
}

/// A renderer whose timeouts fire only when the test says, and what it announces.
@MainActor
private func renderer(_ drawer: FakeDrawer, clock: TimeoutClock = TimeoutClock(), limit: Int = 48 << 20) -> (MermaidRenderer, Announced) {
	let renderer = MermaidRenderer(drawer: drawer, limit: limit, sleep: clock.sleep)
	return (renderer, Announced(renderer))
}

// The waits are for events, not times; the limit only catches a hang.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct MermaidRendererTests {
	@Test func diagramsAreDrawnOneAtATimeAndEachIsAnnounced() async {
		let drawer = FakeDrawer()
		let (renderer, announced) = renderer(drawer)
		for source in ["a", "b", "c"] { renderer.request(key(source), theme: [:]) }
		#expect(renderer.outcome(key("b")) == .pending)
		await announced.count(3)
		#expect(announced.keys.map(\.source) == ["a", "b", "c"])
		#expect(drawer.mostAtOnce == 1)
		guard case .drawn(let image) = renderer.outcome(key("a")) else { return #expect(Bool(false), "a must be drawn") }
		#expect(image.image.size.width == 300)
	}

	@Test func aDiagramAskedForTwiceIsDrawnOnce() async {
		let drawer = FakeDrawer()
		let (renderer, announced) = renderer(drawer)
		renderer.request(key("a"), theme: [:])
		renderer.request(key("a"), theme: [:])
		await announced.done(key("a"))
		renderer.request(key("a"), theme: [:])
		// A diagram asked for after it, drawn in order: had the third ask drawn a again, it would be in between.
		renderer.request(key("b"), theme: [:])
		await announced.done(key("b"))
		#expect(drawer.drawn == ["a", "b"])
	}

	@Test func aFailedDiagramFails() async {
		let drawer = FakeDrawer()
		drawer.failing = ["bad"]
		let (renderer, announced) = renderer(drawer)
		renderer.request(key("bad"), theme: [:])
		await announced.done(key("bad"))
		#expect(renderer.outcome(key("bad")) == .failed)
	}

	@Test func aDiagramThatNeverFinishesTimesOutAndTheNextOneStillDraws() async throws {
		let drawer = FakeDrawer()
		drawer.hanging = ["stuck"]
		let clock = TimeoutClock()
		let (renderer, announced) = renderer(drawer, clock: clock)
		renderer.request(key("stuck"), theme: [:])
		renderer.request(key("next"), theme: [:])
		try await eventually { clock.pending == 1 }
		clock.fire()  // stuck's timeout: next's hasn't started
		await announced.done(key("next"))
		#expect(renderer.outcome(key("stuck")) == .failed, "A timeout is a failure: the row shows the source")
		#expect(drawer.resets == 1, "The stuck page is dropped, so the next diagram gets a fresh one")
		guard case .drawn = renderer.outcome(key("next")) else { return #expect(Bool(false), "next must be drawn") }
	}

	@Test func theCacheKeepsTheMostRecentlyShownWithinItsBound() async {
		// Each fake drawing is 300 × 100 points at scale 2, 600 × 200 pixels: 480,000 bytes, so two fit and three don't.
		let (renderer, announced) = renderer(FakeDrawer(), limit: 1_000_000)
		renderer.request(key("a"), theme: [:])
		renderer.request(key("b"), theme: [:])
		await announced.done(key("b"))
		_ = renderer.outcome(key("a"))  // a is shown again, so b is the least recent
		renderer.request(key("c"), theme: [:])
		await announced.done(key("c"))
		#expect(renderer.outcome(key("b")) == nil, "The least recently shown goes first")
		#expect(renderer.outcome(key("a")) != nil)
	}

	@Test func aNewThemeStartsAFreshPage() async {
		let drawer = FakeDrawer()
		let (renderer, announced) = renderer(drawer)
		renderer.request(key("a"), theme: [:])
		await announced.done(key("a"))
		#expect(drawer.resets == 0)
		let light = key("a", theme: ["--color-bg": "#fff"])
		renderer.request(light, theme: [:])
		await announced.done(light)
		#expect(drawer.resets == 1, "Mermaid takes its colors once, so another theme needs a fresh page")
	}
}

/// The real drawer on the built web renderer (`mise run web-renderer`), as the app bundles it.
@MainActor
@Suite(.serialized, .timeLimit(.minutes(1)), .enabled(if: webRenderer != nil, "needs build/web-renderer (mise run web-renderer)"))
struct WebDiagramDrawerTests {
	/// A renderer on the real drawer, with no timeout of its own: WebKit's answer needs the main actor too, so a timeout
	/// would race the other suites; a page that never answers is the suite's time limit.
	private func realRenderer() -> (MermaidRenderer, Announced) {
		let renderer = MermaidRenderer(drawer: WebDiagramDrawer(root: webRenderer), sleep: TimeoutClock().sleep)
		return (renderer, Announced(renderer))
	}

	@Test func aDiagramIsDrawnSharplyAtItsWidth() async throws {
		let (renderer, announced) = realRenderer()
		let diagram = key("flowchart LR\n  A[Client] --> B[Server]", width: 500, theme: [:])
		let failing = key("flowchart LR\n  A[Client --> (((", width: 500, theme: [:])
		// Two at once: each waits for the page, and neither waits forever.
		renderer.request(diagram, theme: [:])
		renderer.request(failing, theme: [:])
		await announced.count(2)
		guard case .drawn(let drawn) = renderer.outcome(diagram) else { return #expect(Bool(false), "The diagram must be drawn") }
		#expect(drawn.image.size.width > 50 && drawn.image.size.width <= 500, "At most the row's width: \(drawn.image.size)")
		let pixels = try #require(drawn.image.representations.first).pixelsWide
		#expect(abs(CGFloat(pixels) - drawn.image.size.width * 2) <= 2, "At the diagram's scale: \(pixels) pixels for \(drawn.image.size.width) points")
		#expect(renderer.outcome(failing) == .failed, "Source that doesn't parse fails, as on the web")
	}

	@Test func aDiagramIsDrawnAtItsScaleWhateverTheScreens() async throws {
		// This Mac's screen is at most 2×, and a CI runner's 1×: 3× is drawn only if the drawer makes it so.
		let (renderer, announced) = realRenderer()
		let diagram = DiagramKey(source: "flowchart LR\n  A[Client] --> B[Server]", width: 500, scale: 3, theme: [:])
		renderer.request(diagram, theme: [:])
		await announced.done(diagram)
		guard case .drawn(let drawn) = renderer.outcome(diagram) else { return #expect(Bool(false), "The diagram must be drawn") }
		let pixels = try #require(drawn.image.representations.first).pixelsWide
		#expect(abs(CGFloat(pixels) - drawn.image.size.width * 3) <= 3, "\(pixels) pixels for \(drawn.image.size.width) points")
	}
}

/// The built renderer, if there is one: `build/web-renderer`, beside this package's `Tests`.
private let webRenderer: URL? = {
	let root = URL(filePath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
		.appending(path: "build/web-renderer", directoryHint: .isDirectory)
	return FileManager.default.fileExists(atPath: root.appending(path: "index.html").path) ? root : nil
}()

private func row(_ sequence: UInt64, _ text: String, state: ItemState = .complete) -> TranscriptItem {
	TranscriptItem(sequence: sequence, cliSession: "s", parentToolUseId: nil, state: state, content: .assistant(text: text))
}

private let diagramReply = "A diagram:\n\n```mermaid\nflowchart LR\n  A --> B\n```"

private let diagramSource = "flowchart LR\n  A --> B"

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct TranscriptDiagramTests {
	private struct Shown {
		let window: NSWindow
		let view: TranscriptScrollView
		let renderer: MermaidRenderer
		let announced: Announced
	}

	private func shown(_ items: [TranscriptItem], drawer: FakeDrawer) -> Shown {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let (renderer, announced) = renderer(drawer)
		let view = TranscriptScrollView(anchor: nil, renderer: renderer)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply(items)
		return Shown(window: window, view: view, renderer: renderer, announced: announced)
	}

	private func diagrams(_ view: TranscriptScrollView) -> [String: Int] {
		view.observation()["diagrams"] as? [String: Int] ?? [:]
	}

	@Test func aDiagramShowsAPlaceholderThenItsImageAtTheBottom() async throws {
		let shown = shown((1...30).map { row($0, "Reply \($0).") } + [row(31, diagramReply)], drawer: FakeDrawer())
		defer { shown.window.close() }
		#expect(diagrams(shown.view)["pending"] == 1)
		let placeholder = shown.view.table.rect(ofRow: 30).height
		await shown.announced.count(1)
		try await eventually { diagrams(shown.view)["drawn"] == 1 }
		#expect(shown.view.table.rect(ofRow: 30).height != placeholder, "The image's height replaces the placeholder's")
		#expect(shown.view.distanceFromBottom == 0, "Following the bottom keeps the bottom through the correction")
	}

	@Test func aDiagramDrawnAboveDoesntMoveTheRowsTheReaderSees() async throws {
		let drawer = FakeDrawer()
		let gate = Gate()
		drawer.gate = gate
		var items = (1...40).map { row($0, "Reply \($0).") }
		items[19] = row(20, diagramReply)
		let shown = shown(items, drawer: drawer)
		defer { shown.window.close() }
		// The diagram's row just above the top, still a placeholder (its draw held open), and the reader on the rows below it.
		let diagramRow = shown.view.table.rect(ofRow: 19)
		shown.view.contentView.scroll(to: NSPoint(x: 0, y: diagramRow.maxY - 10))
		shown.view.reflectScrolledClipView(shown.view.contentView)
		try await eventually { drawer.atOnce == 1 }
		let reading = shown.view.table.rect(ofRow: 20).minY - shown.view.contentView.bounds.minY
		gate.open()
		await shown.announced.count(1)
		try await eventually { diagrams(shown.view)["drawn"] == 1 }
		#expect(shown.view.table.rect(ofRow: 20).minY - shown.view.contentView.bounds.minY == reading, "The row under the diagram stays put")
	}

	@Test func aStreamingReplyShowsItsSourceAndAsksForNothing() async throws {
		let shown = shown([row(1, diagramReply, state: .streaming)], drawer: FakeDrawer())
		defer { shown.window.close() }
		#expect(diagrams(shown.view)["source"] == 1)
		#expect(shown.renderer.outcome(shown.view.diagramKey(diagramSource)) == nil, "Never asked for, while it streams")
		// Once complete, it is drawn.
		shown.view.apply([row(1, diagramReply)])
		await shown.announced.count(1)
		try await eventually { diagrams(shown.view)["drawn"] == 1 }
	}

	@Test func aFailedDiagramShowsItsSource() async throws {
		let drawer = FakeDrawer()
		drawer.failing = [diagramSource]
		let shown = shown([row(1, diagramReply)], drawer: drawer)
		defer { shown.window.close() }
		await shown.announced.count(1)
		try await eventually { diagrams(shown.view)["failed"] == 1 }
		#expect((shown.view.observation()["text"] as? String)?.contains("A --> B") == true)
	}
}
