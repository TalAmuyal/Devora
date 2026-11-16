import CoreGraphics
import XCTest

final class GlobalPauseUITests: XCTestCase {
	@MainActor
	func testToolbarHoldSurvivesRelaunchAndThePaletteResumesIt() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-global-pause-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		let config = try writeConfigWithAProfile(in: scratch)
		let app = XCUIApplication()
		app.allowSlowShiftDoubleTaps()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		let inputTrace = scratch.appending(path: "input-trace.log")
		app.launchEnvironment["DEVORA_INPUT_TRACE"] = inputTrace.path
		app.launch()
		addTeardownBlock { @MainActor in
			app.terminate()
			try? FileManager.default.removeItem(at: scratch)
		}
		app.openFirstShell()
		let toggle = app.buttons["global-pause-toggle"]
		XCTAssertTrue(toggle.waitForExistence(timeout: 10))
		toggle.click()
		let paused = app.staticTexts["global-pause-indicator"]
		XCTAssertTrue(paused.waitForExistence(timeout: 5))
		app.terminate()
		app.launch()
		XCTAssertTrue(paused.waitForExistence(timeout: 10), "The toolbar restores the persisted hold")
		XCUIElement.perform(withKeyModifiers: .shift) {}
		XCUIElement.perform(withKeyModifiers: .shift) {}
		let search = app.descendants(matching: .any)["command-palette-search"].firstMatch
		if !search.waitForExistence(timeout: 5) {
			// What the app made of the two taps: whether they arrived, for which window, and how the double tap read them.
			// Beside it, whether the system still has Shift down: if so, the last release was never posted; if not, it was posted and the app never got it.
			// (Synthesized events change the session's state only, not the HID system's.)
			let appTrace = (try? String(contentsOf: inputTrace, encoding: .utf8)) ?? "(no input trace)"
			let shiftDown = CGEventSource.flagsState(.combinedSessionState).contains(.maskShift)
			let trace = XCTAttachment(string: "\(appTrace)system now: Shift down \(shiftDown)\n")
			trace.name = "The app's shortcut decisions"
			trace.lifetime = .keepAlways
			add(trace)
			XCTFail("Shift-Shift after the relaunch must open the command palette")
			return
		}
		app.typeText("Resume Devora")
		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(paused.waitForNonExistence(timeout: 5))
		XCTAssertTrue(toggle.label.contains("Pause Devora"))
		let capture = XCTAttachment(screenshot: app.screenshot())
		capture.name = "Global Pause toolbar after palette resume"
		capture.lifetime = .keepAlways
		add(capture)
	}
}
