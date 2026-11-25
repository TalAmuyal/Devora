import CoreGraphics
import XCTest

/// What a real Shift-Shift that hasn't opened the palette yet calls for.
/// XCUITest sometimes never posts a Shift release, relaunch or not: the system still has Shift down, so the app never saw the gesture end.
/// Only that measured case is XCUITest's, and gets the one retry; a gesture whose releases all arrived and didn't open the palette is the app's failure.
enum ShiftShiftCheck: Equatable {
	case opened, retry, failed

	static func after(opened: Bool, systemShiftDown: Bool, retried: Bool) -> Self {
		if opened { return .opened }
		return systemShiftDown && !retried ? .retry : .failed
	}
}

extension XCTestCase {
	/// Taps Shift twice with real modifier events, types `text` right after, and fails with `failure` unless `field` appears.
	/// When the system still has Shift down, the gesture is sent once more, which posts the missing release first; the retry is printed and attached ("Shift retry: …"), so a runner that needs it often shows.
	/// On failure, it attaches what the app made of the taps (`DEVORA_INPUT_TRACE`): whether they arrived, for which window, and how the double tap read them, with whether the system still has Shift down.
	@MainActor
	func shiftShift(in app: XCUIApplication, typing text: String, opens field: XCUIElement, inputTrace: URL, failure: String, file: StaticString = #filePath, line: UInt = #line) {
		var retried = false
		while true {
			XCUIElement.perform(withKeyModifiers: .shift) {}
			XCUIElement.perform(withKeyModifiers: .shift) {}
			if !text.isEmpty { app.typeText(text) }
			let opened = field.waitForExistence(timeout: 5)
			let shiftDown = CGEventSource.flagsState(.combinedSessionState).contains(.maskShift)
			let appTrace = (try? String(contentsOf: inputTrace, encoding: .utf8)) ?? "(no input trace)"
			switch ShiftShiftCheck.after(opened: opened, systemShiftDown: shiftDown, retried: retried) {
			case .opened:
				return
			case .retry:
				let note = "Shift retry: \(failure): the system still had Shift down, so XCUITest never posted a release"
				print(note)
				let attachment = XCTAttachment(string: "\(note)\n\(appTrace)")
				attachment.name = note
				attachment.lifetime = .keepAlways
				add(attachment)
				retried = true
			case .failed:
				let trace = XCTAttachment(string: "\(appTrace)system now: Shift down \(shiftDown)\n")
				trace.name = "The app's shortcut decisions"
				trace.lifetime = .keepAlways
				add(trace)
				XCTFail("\(failure)\n\(app.debugDescription)", file: file, line: line)
				return
			}
		}
	}
}

/// The retry rule alone, without the app.
final class ShiftShiftCheckTests: XCTestCase {
	func testOnlyAShiftTheSystemStillHoldsIsRetriedAndOnlyOnce() {
		XCTAssertEqual(ShiftShiftCheck.after(opened: true, systemShiftDown: true, retried: false), .opened)
		XCTAssertEqual(ShiftShiftCheck.after(opened: true, systemShiftDown: false, retried: true), .opened)
		XCTAssertEqual(ShiftShiftCheck.after(opened: false, systemShiftDown: true, retried: false), .retry)
		XCTAssertEqual(ShiftShiftCheck.after(opened: false, systemShiftDown: false, retried: false), .failed, "every release arrived: the app's failure")
		XCTAssertEqual(ShiftShiftCheck.after(opened: false, systemShiftDown: true, retried: true), .failed, "one retry")
	}
}
