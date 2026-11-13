import Foundation
import Layers

/// Tracks lone Shift releases using event timestamps, independently of AppKit's monitor.
struct ShiftDoubleTap {
	/// How soon after the first lone release the second one must come.
	let window: TimeInterval

	static let defaultWindow: TimeInterval = 0.5

	init(window: TimeInterval = defaultWindow) {
		self.window = window
	}

	/// The window `DEVORA_TEST_SHIFT_TAP_WINDOW` sets, in seconds, or the default when it is unset or not a positive number.
	/// The XCUITests set a long one: their synthesized taps can arrive seconds apart on a busy machine, while a keyboard's timestamps are the hardware's.
	static func window(from environment: [String: String]) -> TimeInterval {
		guard let value = environment["DEVORA_TEST_SHIFT_TAP_WINDOW"].flatMap(TimeInterval.init), value.isFinite, value > 0 else { return defaultWindow }
		return value
	}

	private var heldShift: UInt16?
	private var overlappingShifts = false
	private var firstRelease: TimeInterval?

	/// A key-down invalidates both a pending tap and a Shift currently held for a chord.
	mutating func reset() {
		heldShift = nil
		overlappingShifts = false
		firstRelease = nil
	}

	/// Returns true on the second lone release within the window. A blocked surface forgets the entire gesture.
	mutating func flagsChanged(keyCode: UInt16, modifiers: KeyModifiers, at time: TimeInterval, available: Bool) -> Bool {
		guard available else { reset(); return false }
		guard modifiers.isSubset(of: .shift) else { reset(); return false }
		guard keyCode == KeyCode.leftShift || keyCode == KeyCode.rightShift else { reset(); return false }
		if overlappingShifts {
			if !modifiers.contains(.shift) { reset() }
			return false
		}
		if modifiers.contains(.shift) {
			if let heldShift, heldShift != keyCode {
				self.heldShift = nil
				firstRelease = nil
				overlappingShifts = true
			} else {
				heldShift = keyCode
			}
			return false
		}
		guard heldShift == keyCode else { reset(); return false }
		heldShift = nil
		if let firstRelease, time >= firstRelease, time - firstRelease <= window {
			self.firstRelease = nil
			return true
		}
		firstRelease = time
		return false
	}
}
