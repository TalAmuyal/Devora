import AppKit

/// Which scroll-wheel events over the transcript scroll the transcript itself, rather than what is under the pointer.
///
/// A row's code block scrolls only sideways, and would otherwise swallow a vertical swipe that starts over it. So a mostly
/// vertical gesture goes to the transcript wherever it starts, and a mostly horizontal one to what is under the pointer.
/// A trackpad gesture's direction is decided once, at its start, and kept through its momentum, so a slightly diagonal
/// swipe doesn't switch between the two; a mouse wheel's ticks are decided one by one.
struct WheelRouting {
	private var vertical: Bool?

	mutating func toTranscript(phase: NSEvent.Phase, momentumPhase: NSEvent.Phase, deltaX: CGFloat, deltaY: CGFloat) -> Bool {
		if phase.isEmpty && momentumPhase.isEmpty { return abs(deltaY) >= abs(deltaX) }
		if phase.contains(.mayBegin) {
			// Fingers down, not moving yet: nothing to scroll, and the direction is still unknown.
			vertical = nil
			return false
		}
		if phase.contains(.began) {
			// The transcript is the main scroller, so a start with no movement yet goes to it.
			vertical = abs(deltaY) >= abs(deltaX)
		}
		return vertical ?? (abs(deltaY) >= abs(deltaX))
	}
}
