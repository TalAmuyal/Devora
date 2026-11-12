import AppKit
import CoreBindings
import Foundation
import Testing

@testable import AppShell

/// A drawer that takes `delay` per diagram, fails the sources it is told to, never answers for others, and records
/// how many draws ran at once.
@MainActor
private final class FakeDrawer: DiagramDrawer {
	var delay: Duration = .milliseconds(20)
	var failing: Set<String> = []
	var hanging: Set<String> = []
	private(set) var drawn: [String] = []
	private(set) var resets = 0
	private(set) var mostAtOnce = 0
	private var atOnce = 0

	struct Failure: Error {}

	func draw(_ key: DiagramKey, theme: [String: String]) async throws -> NSImage {
		atOnce += 1
		mostAtOnce = max(mostAtOnce, atOnce)
		defer { atOnce -= 1 }
		if hanging.contains(key.source) { try await Task.sleep(for: .seconds(60)) }
		try await Task.sleep(for: delay)
		drawn.append(key.source)
		if failing.contains(key.source) { throw Failure() }
		return NSImage(size: CGSize(width: CGFloat(key.width), height: 100))
	}

	func reset() { resets += 1 }
}

private func key(_ source: String, width: Int = 300, theme: [String: String] = ["--color-bg": "#000"]) -> DiagramKey {
	DiagramKey(source: source, width: width, scale: 2, theme: theme)
}

/// Waits (yielding the main actor, as the renderer's work needs it) until `condition` holds, for up to 5 s.
@MainActor
private func eventually(_ condition: () -> Bool) async -> Bool {
	let deadline = Date().addingTimeInterval(5)
	while !condition() {
		if Date() > deadline { return false }
		try? await Task.sleep(for: .milliseconds(5))
	}
	return true
}

@MainActor
struct MermaidRendererTests {
	@Test func diagramsAreDrawnOneAtATimeAndEachIsAnnounced() async {
		let drawer = FakeDrawer()
		let renderer = MermaidRenderer(drawer: drawer)
		var announced: [String] = []
		_ = renderer.observe { announced.append($0.source) }
		for source in ["a", "b", "c"] { renderer.request(key(source), theme: [:]) }
		#expect(renderer.outcome(key("b")) == .pending)
		#expect(await eventually { announced.count == 3 })
		#expect(announced == ["a", "b", "c"])
		#expect(drawer.mostAtOnce == 1)
		guard case .drawn(let image) = renderer.outcome(key("a")) else { return #expect(Bool(false), "a must be drawn") }
		#expect(image.image.size.width == 300)
	}

	@Test func aDiagramAskedForTwiceIsDrawnOnce() async {
		let drawer = FakeDrawer()
		let renderer = MermaidRenderer(drawer: drawer)
		renderer.request(key("a"), theme: [:])
		renderer.request(key("a"), theme: [:])
		#expect(await eventually { renderer.outcome(key("a")) != .pending })
		renderer.request(key("a"), theme: [:])
		try? await Task.sleep(for: .milliseconds(50))
		#expect(drawer.drawn == ["a"])
	}

	@Test func aFailedDiagramFails() async {
		let drawer = FakeDrawer()
		drawer.failing = ["bad"]
		let renderer = MermaidRenderer(drawer: drawer)
		renderer.request(key("bad"), theme: [:])
		#expect(await eventually { renderer.outcome(key("bad")) == .failed })
	}

	@Test func aDiagramThatNeverFinishesTimesOutAndTheNextOneStillDraws() async {
		let drawer = FakeDrawer()
		drawer.hanging = ["stuck"]
		let renderer = MermaidRenderer(drawer: drawer, timeout: .milliseconds(100))
		renderer.request(key("stuck"), theme: [:])
		renderer.request(key("next"), theme: [:])
		#expect(await eventually { renderer.outcome(key("next")).map { $0 != .pending } ?? false })
		#expect(renderer.outcome(key("stuck")) == .failed, "A timeout is a failure: the row shows the source")
		#expect(drawer.resets == 1, "The stuck page is dropped, so the next diagram gets a fresh one")
		guard case .drawn = renderer.outcome(key("next")) else { return #expect(Bool(false), "next must be drawn") }
	}

	@Test func theCacheKeepsTheMostRecentlyShownWithinItsBound() async {
		// Each fake drawing is 300 × 100 points at scale 2, 600 × 200 pixels: 480,000 bytes, so two fit and three don't.
		let renderer = MermaidRenderer(drawer: FakeDrawer(), limit: 1_000_000)
		renderer.request(key("a"), theme: [:])
		renderer.request(key("b"), theme: [:])
		#expect(await eventually { renderer.outcome(key("b")).map { $0 != .pending } ?? false })
		_ = renderer.outcome(key("a"))  // a is shown again, so b is the least recent
		renderer.request(key("c"), theme: [:])
		#expect(await eventually { renderer.outcome(key("c")).map { $0 != .pending } ?? false })
		#expect(renderer.outcome(key("b")) == nil, "The least recently shown goes first")
		#expect(renderer.outcome(key("a")) != nil)
	}

	@Test func aNewThemeStartsAFreshPage() async {
		let drawer = FakeDrawer()
		let renderer = MermaidRenderer(drawer: drawer)
		renderer.request(key("a"), theme: [:])
		#expect(await eventually { renderer.outcome(key("a")).map { $0 != .pending } ?? false })
		#expect(drawer.resets == 0)
		let light = key("a", theme: ["--color-bg": "#fff"])
		renderer.request(light, theme: [:])
		#expect(await eventually { renderer.outcome(light).map { $0 != .pending } ?? false })
		#expect(drawer.resets == 1, "Mermaid takes its colors once, so another theme needs a fresh page")
	}
}

/// The real drawer on the built web renderer (`mise run web-renderer`), as the app bundles it.
@MainActor
@Suite(.serialized, .enabled(if: webRenderer != nil, "needs build/web-renderer (mise run web-renderer)"))
struct WebDiagramDrawerTests {
	@Test func aDiagramIsDrawnSharplyAtItsWidth() async throws {
		let renderer = MermaidRenderer(drawer: WebDiagramDrawer(root: webRenderer), timeout: .seconds(20))
		let diagram = key("flowchart LR\n  A[Client] --> B[Server]", width: 500, theme: [:])
		let failing = key("flowchart LR\n  A[Client --> (((", width: 500, theme: [:])
		// Two at once: each waits for the page, and neither waits forever.
		renderer.request(diagram, theme: [:])
		renderer.request(failing, theme: [:])
		#expect(await eventually(seconds: 30) { renderer.outcome(failing).map { $0 != .pending } ?? false })
		guard case .drawn(let drawn) = renderer.outcome(diagram) else { return #expect(Bool(false), "The diagram must be drawn") }
		#expect(drawn.image.size.width > 50 && drawn.image.size.width <= 500, "At most the row's width: \(drawn.image.size)")
		let pixels = try #require(drawn.image.representations.first).pixelsWide
		#expect(CGFloat(pixels) >= drawn.image.size.width * 2 - 1, "At the screen's scale: \(pixels) pixels for \(drawn.image.size.width) points")
		#expect(renderer.outcome(failing) == .failed, "Source that doesn't parse fails, as on the web")
	}
}

/// The built renderer, if there is one: `build/web-renderer`, beside this package's `Tests`.
private let webRenderer: URL? = {
	let root = URL(filePath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
		.appending(path: "build/web-renderer", directoryHint: .isDirectory)
	return FileManager.default.fileExists(atPath: root.appending(path: "index.html").path) ? root : nil
}()

@MainActor
private func eventually(seconds: Double, _ condition: () -> Bool) async -> Bool {
	let deadline = Date().addingTimeInterval(seconds)
	while !condition() {
		if Date() > deadline { return false }
		try? await Task.sleep(for: .milliseconds(20))
	}
	return true
}

private func row(_ sequence: UInt64, _ text: String, state: ItemState = .complete) -> TranscriptItem {
	TranscriptItem(sequence: sequence, cliSession: "s", parentToolUseId: nil, state: state, content: .assistant(text: text))
}

private let diagramReply = "A diagram:\n\n```mermaid\nflowchart LR\n  A --> B\n```"

@MainActor
struct TranscriptDiagramTests {
	private func shown(_ items: [TranscriptItem], drawer: FakeDrawer) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: nil, renderer: MermaidRenderer(drawer: drawer))
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.apply(items)
		return (window, view)
	}

	private func diagrams(_ view: TranscriptScrollView) -> [String: Int] {
		view.observation()["diagrams"] as? [String: Int] ?? [:]
	}

	@Test func aDiagramShowsAPlaceholderThenItsImageAtTheBottom() async {
		let drawer = FakeDrawer()
		let (window, view) = shown((1...30).map { row($0, "Reply \($0).") } + [row(31, diagramReply)], drawer: drawer)
		defer { window.close() }
		#expect(diagrams(view)["pending"] == 1)
		let placeholder = view.table.rect(ofRow: 30).height
		#expect(await eventually { diagrams(view)["drawn"] == 1 })
		#expect(view.table.rect(ofRow: 30).height != placeholder, "The image's height replaces the placeholder's")
		#expect(view.distanceFromBottom == 0, "Following the bottom keeps the bottom through the correction")
	}

	@Test func aDiagramDrawnAboveDoesntMoveTheRowsTheReaderSees() async {
		let drawer = FakeDrawer()
		drawer.delay = .milliseconds(200)
		var items = (1...40).map { row($0, "Reply \($0).") }
		items[19] = row(20, diagramReply)
		let (window, view) = shown(items, drawer: drawer)
		defer { window.close() }
		// The diagram's row just above the top, still a placeholder, and the reader on the rows below it.
		let diagramRow = view.table.rect(ofRow: 19)
		view.contentView.scroll(to: NSPoint(x: 0, y: diagramRow.maxY - 10))
		view.reflectScrolledClipView(view.contentView)
		#expect(await eventually { !drawer.drawn.isEmpty || diagrams(view)["pending"] == 1 })
		let reading = view.table.rect(ofRow: 20).minY - view.contentView.bounds.minY
		#expect(await eventually { diagrams(view)["drawn"] == 1 })
		#expect(view.table.rect(ofRow: 20).minY - view.contentView.bounds.minY == reading, "The row under the diagram stays put")
	}

	@Test func aStreamingReplyShowsItsSourceAndAsksForNothing() async {
		let drawer = FakeDrawer()
		let (window, view) = shown([row(1, diagramReply, state: .streaming)], drawer: drawer)
		defer { window.close() }
		#expect(diagrams(view)["source"] == 1)
		try? await Task.sleep(for: .milliseconds(100))
		#expect(drawer.drawn.isEmpty)
		// Once complete, it is drawn.
		view.apply([row(1, diagramReply)])
		#expect(await eventually { diagrams(view)["drawn"] == 1 })
	}

	@Test func aFailedDiagramShowsItsSource() async {
		let drawer = FakeDrawer()
		drawer.failing = ["flowchart LR\n  A --> B"]
		let (window, view) = shown([row(1, diagramReply)], drawer: drawer)
		defer { window.close() }
		#expect(await eventually { diagrams(view)["failed"] == 1 })
		#expect((view.observation()["text"] as? String)?.contains("A --> B") == true)
	}
}

