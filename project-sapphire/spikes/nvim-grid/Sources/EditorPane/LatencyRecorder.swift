import AppKit
import QuartzCore

/// Keystroke-to-glyph timing, one key in flight at a time (the bench spaces keys out). All times are
/// `CACurrentMediaTime()` seconds, the same clock as `NSEvent.timestamp` and `CADisplayLink.targetTimestamp`.
@MainActor
public final class LatencyRecorder {
	public struct Sample: Sendable, Codable {
		/// Input sent → the `flush` carrying its redraw reached the main actor (RPC + Neovim + parsing).
		public var flushMs: Double
		/// Input sent → the grid view finished drawing that flush.
		public var drawnMs: Double
		/// Input sent → the display refresh that shows it (`targetTimestamp` of the first vsync after drawing).
		public var presentedMs: Double
	}

	private var sent: CFTimeInterval?
	private var flushedAt: CFTimeInterval?
	private var drawnAt: CFTimeInterval?
	/// `targetTimestamp` of the latest display-link callback: when the frame being built right now reaches the screen.
	private var currentTarget: CFTimeInterval = 0
	public private(set) var samples: [Sample] = []
	/// NSEvent timestamp → keyDown, for real keystrokes only.
	public private(set) var eventAgesMs: [Double] = []
	/// Sends keys through the grid view's input path, for scripted typing.
	public internal(set) var inject: ((String) -> Void)?

	public init() {}

	public func reset() {
		samples.removeAll()
		eventAgesMs.removeAll()
		sent = nil
	}

	func keySent() {
		sent = CACurrentMediaTime()
		flushedAt = nil
		drawnAt = nil
	}

	func keyEventAge(_ event: NSEvent) {
		eventAgesMs.append((CACurrentMediaTime() - event.timestamp) * 1000)
	}

	func flushed() {
		if sent != nil, flushedAt == nil { flushedAt = CACurrentMediaTime() }
	}

	func drawn() {
		if flushedAt != nil, drawnAt == nil {
			drawnAt = CACurrentMediaTime()
			frameAtDraw = currentTarget
		}
	}

	func vsync(target: CFTimeInterval) {
		currentTarget = target
		// Drawn after this cycle's callback, but before its target: it is in this frame. Otherwise the next one.
		guard let sent, let flushedAt, let drawnAt else { return }
		samples.append(Sample(
			flushMs: (flushedAt - sent) * 1000, drawnMs: (drawnAt - sent) * 1000,
			presentedMs: (presentedTarget(drawnAt: drawnAt, next: target) - sent) * 1000))
		self.sent = nil
	}

	/// The frame a drawing at `drawnAt` lands in: the update cycle it happened in, if that cycle's frame was still
	/// ahead, else the following one.
	private func presentedTarget(drawnAt: CFTimeInterval, next: CFTimeInterval) -> CFTimeInterval {
		drawnAt <= frameAtDraw ? frameAtDraw : next
	}

	private var frameAtDraw: CFTimeInterval = 0
}
