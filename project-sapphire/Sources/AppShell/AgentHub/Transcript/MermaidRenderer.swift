import AppKit
import WebKit

/// A Mermaid diagram to draw: its source, at a width in points and a screen scale, in a theme (its colors' hash).
struct DiagramKey: Hashable {
	let source: String
	let width: Int
	let scale: CGFloat
	let theme: Int

	init(source: String, width: Int, scale: CGFloat, theme: [String: String]) {
		self.source = source
		self.width = width
		self.scale = scale
		self.theme = Self.hash(theme)
	}

	static func hash(_ theme: [String: String]) -> Int {
		var hasher = Hasher()
		for (name, value) in theme.sorted(by: { $0.key < $1.key }) {
			hasher.combine(name)
			hasher.combine(value)
		}
		return hasher.finalize()
	}
}

/// What a diagram row shows: a placeholder until its image is drawn, the image, or its source after a failure.
enum DiagramOutcome: Equatable {
	case pending
	case drawn(DiagramImage)
	case failed
}

/// A drawn diagram; two are the same only if they are the same drawing.
struct DiagramImage: Equatable {
	let image: NSImage

	static func == (lhs: Self, rhs: Self) -> Bool { lhs.image === rhs.image }
}

/// Draws one diagram at a time (the renderer never asks for two at once).
@MainActor
protocol DiagramDrawer: AnyObject {
	func draw(_ key: DiagramKey, theme: [String: String]) async throws -> NSImage
	/// Drops whatever is drawing, since it took too long or the theme changed: the next draw starts afresh.
	func reset()
}

/// One shared offscreen renderer for the native transcript's Mermaid diagrams (ADR-014): it draws them one at a time,
/// keeps the drawn ones in a cache bounded by their pixels, and tells its observers when one is done.
///
/// A diagram that fails, or doesn't finish within `timeout`, fails: its row shows the source, as the web transcript does.
@MainActor
final class MermaidRenderer {
	static let shared = MermaidRenderer(drawer: WebDiagramDrawer())

	private let drawer: any DiagramDrawer
	private let timeout: Duration
	private let limit: Int
	private var cache: [DiagramKey: (outcome: DiagramOutcome, bytes: Int)] = [:]
	/// The cache's keys, least recently used first.
	private var recency: [DiagramKey] = []
	private var bytes = 0
	private var queue: [(key: DiagramKey, theme: [String: String])] = []
	private var working = false
	private var drawnTheme: Int?
	private var observers: [UUID: (DiagramKey) -> Void] = [:]
	private(set) var draws = 0

	init(drawer: any DiagramDrawer, timeout: Duration = .seconds(5), limit: Int = 48 << 20) {
		self.drawer = drawer
		self.timeout = timeout
		self.limit = limit
	}

	/// The diagram's outcome if it was drawn (or failed); nil if it was never asked for, or was dropped from the cache.
	func outcome(_ key: DiagramKey) -> DiagramOutcome? {
		guard let entry = cache[key] else { return queue.contains { $0.key == key } ? .pending : nil }
		touch(key)
		return entry.outcome
	}

	/// Queues the diagram, unless it is drawn already or on its way.
	func request(_ key: DiagramKey, theme: [String: String]) {
		guard cache[key] == nil, !queue.contains(where: { $0.key == key }) else { return }
		queue.append((key, theme))
		guard !working else { return }
		working = true
		Task { await work() }
	}

	/// Calls `handler` with each diagram as it is done; the token stops it.
	func observe(_ handler: @escaping (DiagramKey) -> Void) -> UUID {
		let token = UUID()
		observers[token] = handler
		return token
	}

	func stopObserving(_ token: UUID) {
		observers[token] = nil
	}

	private func work() async {
		while let next = queue.first {
			let outcome = await draw(next.key, theme: next.theme)
			queue.removeFirst()
			store(next.key, outcome)
			for handler in observers.values { handler(next.key) }
		}
		working = false
	}

	private func draw(_ key: DiagramKey, theme: [String: String]) async -> DiagramOutcome {
		// Mermaid takes its colors once, when it starts: a new theme needs a fresh page.
		if let drawnTheme, drawnTheme != key.theme { drawer.reset() }
		drawnTheme = key.theme
		draws += 1
		let race = Race()
		return await withCheckedContinuation { continuation in
			Task {
				let outcome: DiagramOutcome
				do { outcome = .drawn(DiagramImage(image: try await drawer.draw(key, theme: theme))) } catch { outcome = .failed }
				if race.finish() { continuation.resume(returning: outcome) }
			}
			Task {
				try? await Task.sleep(for: timeout)
				guard race.finish() else { return }
				// The page may never answer: the next diagram gets a fresh one rather than waiting behind it.
				drawer.reset()
				continuation.resume(returning: .failed)
			}
		}
	}

	private func store(_ key: DiagramKey, _ outcome: DiagramOutcome) {
		let size: Int
		if case .drawn(let drawn) = outcome {
			let pixels = drawn.image.size.width * key.scale * drawn.image.size.height * key.scale
			size = Int(pixels) * 4
		} else {
			size = 0
		}
		cache[key] = (outcome, size)
		bytes += size
		touch(key)
		// The least recently shown go first, but never the one just drawn.
		while bytes > limit, let oldest = recency.first, oldest != key {
			recency.removeFirst()
			bytes -= cache.removeValue(forKey: oldest)?.bytes ?? 0
		}
	}

	private func touch(_ key: DiagramKey) {
		recency.removeAll { $0 == key }
		recency.append(key)
	}

	/// Which of a draw and its timeout came first.
	private final class Race {
		private var done = false

		func finish() -> Bool {
			defer { done = true }
			return !done
		}
	}
}

/// Draws diagrams in a hidden web view on the bundled renderer, the one the web transcript and previews use, at the
/// diagram's width and scale.
@MainActor
final class WebDiagramDrawer: NSObject, DiagramDrawer, WKScriptMessageHandler {
	private let root: URL?
	private var window: NSWindow?
	private var webView: WKWebView?
	private var ready = false
	private var waiting: [CheckedContinuation<Void, Never>] = []

	init(root: URL? = Bundle.main.resourceURL?.appending(path: "web-renderer", directoryHint: .isDirectory)) {
		self.root = root
	}

	struct Failure: Error {}

	func draw(_ key: DiagramKey, theme: [String: String]) async throws -> NSImage {
		let view = await loadedView(scale: key.scale)
		resize(view, width: CGFloat(key.width), height: 600)
		try await view.callAsyncJavaScript(
			"await window.devoraRender(markdown, theme)",
			arguments: ["markdown": "```mermaid\n\(key.source)\n```", "theme": theme], in: nil, contentWorld: .page)
		let found = try await view.callAsyncJavaScript(
			"""
			if (document.querySelector('pre.mermaid-error')) return null;
			const box = document.querySelector('.mermaid-diagram svg')?.getBoundingClientRect();
			return box && box.width > 0 && box.height > 0 ? [box.x, box.y, box.width, box.height] : null;
			""", arguments: [:], in: nil, contentWorld: .page) as? [Double]
		guard let found, found.count == 4 else { throw Failure() }
		let rect = CGRect(x: found[0], y: found[1], width: found[2], height: found[3])
		// A tall diagram needs a tall page, or its bottom isn't drawn.
		if rect.maxY > view.frame.height { resize(view, width: CGFloat(key.width), height: ceil(rect.maxY) + 8) }
		let configuration = WKSnapshotConfiguration()
		configuration.rect = rect
		configuration.afterScreenUpdates = true
		return try await view.takeSnapshot(configuration: configuration)
	}

	func reset() {
		webView?.configuration.userContentController.removeScriptMessageHandler(forName: "devoraRenderer")
		webView?.stopLoading()
		webView = nil
		window?.close()
		window = nil
		ready = false
		// A draw waiting for the page gets the fresh one.
		let waiters = waiting
		waiting = []
		waiters.forEach { $0.resume() }
	}

	/// The page, once its renderer has started; every caller waiting for it is let through together.
	private func loadedView(scale: CGFloat) async -> WKWebView {
		while true {
			if let webView, ready { return webView }
			if webView == nil { load(scale: scale) }
			await withCheckedContinuation { waiting.append($0) }
		}
	}

	private func load(scale: CGFloat) {
		let configuration = WKWebViewConfiguration()
		configuration.setURLSchemeHandler(RendererSchemeHandler(root: root), forURLScheme: "devora-renderer")
		configuration.userContentController.add(self, name: "devoraRenderer")
		let view = WKWebView(frame: CGRect(x: 0, y: 0, width: 600, height: 600), configuration: configuration)
		// An offscreen window gives the page a backing scale, so the drawing is as sharp as the screen.
		let window = OffscreenWindow(scale: scale)
		window.contentView = view
		view.load(URLRequest(url: URL(string: "devora-renderer://app/index.html")!))
		self.window = window
		webView = view
	}

	private func resize(_ view: WKWebView, width: CGFloat, height: CGFloat) {
		window?.setContentSize(CGSize(width: width, height: height))
		view.frame = CGRect(x: 0, y: 0, width: width, height: height)
	}

	func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
		guard message.webView === webView, (message.body as? [String: Any])?["ready"] as? Bool == true else { return }
		ready = true
		let waiters = waiting
		waiting = []
		waiters.forEach { $0.resume() }
	}
}

/// A window that is never shown, at a given backing scale.
private final class OffscreenWindow: NSWindow {
	private let scale: CGFloat

	init(scale: CGFloat) {
		self.scale = scale
		super.init(contentRect: CGRect(x: -20_000, y: -20_000, width: 600, height: 600), styleMask: [.borderless], backing: .buffered, defer: false)
		isReleasedWhenClosed = false
	}

	override var backingScaleFactor: CGFloat { scale }
}
