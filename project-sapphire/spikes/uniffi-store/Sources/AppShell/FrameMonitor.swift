import AppKit
import QuartzCore
import Store
import SwiftUI

/// Frame pacing as seen by a 120 Hz display link on the main thread: a late callback means the main thread missed a frame.
@MainActor
public final class FrameStats {
	public nonisolated static let frameMs = 1000.0 / 120.0

	private var intervals: [Double] = []
	private var last: CFTimeInterval?
	private var recording = false

	public func start() {
		intervals.removeAll(keepingCapacity: true)
		last = nil
		recording = true
	}

	public func stop() -> FrameSummary {
		recording = false
		return FrameSummary(intervals)
	}

	func record(_ timestamp: CFTimeInterval) {
		guard recording else { return }
		if let last { intervals.append((timestamp - last) * 1000) }
		last = timestamp
	}
}

public struct FrameSummary: Codable, Sendable {
	public var frames: Int
	public var averageFPS: Double
	public var intervalMs: Percentiles
	/// Display refreshes with no main-thread frame, estimated from interval lengths.
	public var droppedFrames: Int
	public var droppedPercent: Double

	init(_ intervals: [Double]) {
		frames = intervals.count
		let total = intervals.reduce(0, +)
		averageFPS = total > 0 ? Double(intervals.count) / (total / 1000) : 0
		intervalMs = Percentiles(intervals)
		droppedFrames = intervals.reduce(0) { $0 + max(0, Int(($1 / FrameStats.frameMs).rounded()) - 1) }
		let refreshes = total / FrameStats.frameMs
		droppedPercent = refreshes > 0 ? Double(droppedFrames) / refreshes * 100 : 0
	}
}

/// Drives `Store.frameTick()` and `FrameStats` from the window's display link.
struct FrameDriver: NSViewRepresentable {
	let stats: FrameStats
	let onFrame: @MainActor () -> Void

	func makeNSView(context: Context) -> DisplayLinkView {
		DisplayLinkView(stats: stats, onFrame: onFrame)
	}

	func updateNSView(_ view: DisplayLinkView, context: Context) {}
}

final class DisplayLinkView: NSView {
	private let stats: FrameStats
	private let onFrame: @MainActor () -> Void
	private var link: CADisplayLink?

	init(stats: FrameStats, onFrame: @escaping @MainActor () -> Void) {
		self.stats = stats
		self.onFrame = onFrame
		super.init(frame: .zero)
	}

	@available(*, unavailable)
	required init?(coder: NSCoder) { fatalError() }

	override func viewDidMoveToWindow() {
		super.viewDidMoveToWindow()
		link?.invalidate()
		guard window != nil else { return }
		let link = displayLink(target: self, selector: #selector(tick(_:)))
		link.preferredFrameRateRange = CAFrameRateRange(minimum: 120, maximum: 120, preferred: 120)
		link.add(to: .main, forMode: .common)
		self.link = link
	}

	@objc private func tick(_ link: CADisplayLink) {
		stats.record(link.timestamp)
		onFrame()
	}
}
