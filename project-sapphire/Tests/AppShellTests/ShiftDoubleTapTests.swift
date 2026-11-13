@testable import AppShell
import Foundation
import Layers
import Testing

struct ShiftDoubleTapTests {
	@Test func releaseTimingAndReopening() {
		var tap = ShiftDoubleTap()
		func event(_ modifiers: KeyModifiers, _ time: Double) -> Bool {
			tap.flagsChanged(keyCode: 0x38, modifiers: modifiers, at: time, available: true)
		}
		#expect(!event(.shift, 0))
		#expect(!event([], 0.1))
		#expect(!event(.shift, 0.2))
		#expect(event([], 0.6))
		#expect(!event(.shift, 0.7))
		#expect(!event([], 0.8))
		#expect(!event(.shift, 1.0))
		#expect(event([], 1.1))
	}

	@Test func lateInterruptedAndBlockedTapsDoNotOpen() {
		var tap = ShiftDoubleTap()
		func event(_ modifiers: KeyModifiers, at time: Double, _ available: Bool) -> Bool {
			tap.flagsChanged(keyCode: 0x38, modifiers: modifiers, at: time, available: available)
		}
		#expect(!event( .shift, at: 0, true))
		#expect(!event( [], at: 0.1, true))
		#expect(!event( .shift, at: 0.7, true))
		#expect(!event( [], at: 0.8, true))
		tap.reset()
		#expect(!event( .shift, at: 0.9, true))
		#expect(!event( [], at: 1, true))
		#expect(!event( .shift, at: 1.1, false))
		#expect(!event( [], at: 1.2, true))
		#expect(!event( [.shift, .control], at: 1.3, true))
	}

	@Test func overlappingShiftsAreNotLoneTaps() {
		var tap = ShiftDoubleTap()
		func event(_ code: UInt16, _ modifiers: KeyModifiers, _ time: Double) -> Bool {
			tap.flagsChanged(keyCode: code, modifiers: modifiers, at: time, available: true)
		}
		#expect(!event(0x38, .shift, 0))
		#expect(!event(0x3c, .shift, 0.1))
		#expect(!event(0x38, .shift, 0.2))
		#expect(!event(0x3c, [], 0.3))
		#expect(!event(0x38, .shift, 0.4))
		#expect(!event(0x38, [], 0.5))
		#expect(!event(0x3c, .shift, 0.6))
		#expect(event(0x3c, [], 0.7))
	}
}

struct ShiftDoubleTapWindowTests {
	@Test func aLongerWindowTakesTapsFurtherApart() {
		func opens(window: TimeInterval) -> Bool {
			var tap = ShiftDoubleTap(window: window)
			_ = tap.flagsChanged(keyCode: 0x38, modifiers: .shift, at: 0, available: true)
			_ = tap.flagsChanged(keyCode: 0x38, modifiers: [], at: 0.1, available: true)
			_ = tap.flagsChanged(keyCode: 0x38, modifiers: .shift, at: 1.5, available: true)
			return tap.flagsChanged(keyCode: 0x38, modifiers: [], at: 2.1, available: true)
		}
		#expect(!opens(window: ShiftDoubleTap.defaultWindow))
		#expect(opens(window: 60))
	}

	@Test func theEnvironmentSetsOnlyAPositiveWindow() {
		func window(_ value: String) -> TimeInterval {
			ShiftDoubleTap.window(from: ["DEVORA_TEST_SHIFT_TAP_WINDOW": value])
		}
		#expect(ShiftDoubleTap.window(from: [:]) == 0.5)
		#expect(window("60") == 60)
		#expect(window("1.5") == 1.5)
		for ignored in ["0", "-1", "soon", "", "inf", "nan"] {
			#expect(window(ignored) == 0.5, "\(ignored)")
		}
	}
}
