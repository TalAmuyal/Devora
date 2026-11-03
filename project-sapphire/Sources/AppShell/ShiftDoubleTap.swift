import Foundation
import Layers

/// Tracks lone Shift releases using event timestamps, independently of AppKit's monitor.
struct ShiftDoubleTap {
	private var heldShift: UInt16?
	private var overlappingShifts = false
	private var firstRelease: TimeInterval?

	/// A key-down invalidates both a pending tap and a Shift currently held for a chord.
	mutating func reset() {
		heldShift = nil
		overlappingShifts = false
		firstRelease = nil
	}

	/// Returns true on the second lone release within 500 ms. A blocked surface forgets the entire gesture.
	mutating func flagsChanged(keyCode: UInt16, modifiers: KeyModifiers, at time: TimeInterval, available: Bool) -> Bool {
		guard available else { reset(); return false }
		guard modifiers.isSubset(of: .shift) else { reset(); return false }
		guard keyCode == 0x38 || keyCode == 0x3c else { reset(); return false }
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
		if let firstRelease, time >= firstRelease, time - firstRelease <= 0.5 {
			self.firstRelease = nil
			return true
		}
		firstRelease = time
		return false
	}
}
