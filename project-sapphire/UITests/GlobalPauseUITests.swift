import XCTest

final class GlobalPauseUITests: XCTestCase {
	/// Resumes with the toolbar, not the palette: XCUITest sometimes doesn't post a Shift release right after a relaunch (the system still had Shift down seconds later), so Shift-Shift there would test XCUITest.
	/// Real Shift-Shift is in `KeyRoutingUITests`, and resuming from the palette after a relaunch in `global-pause.feature`.
	@MainActor
	func testToolbarHoldSurvivesRelaunchAndTheToolbarResumesIt() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-global-pause-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		let config = try writeConfigWithAProfile(in: scratch)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
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
		XCTAssertTrue(toggle.label.contains("Resume Devora"), "The restored toolbar offers to resume")
		toggle.click()
		XCTAssertTrue(paused.waitForNonExistence(timeout: 5))
		XCTAssertTrue(toggle.label.contains("Pause Devora"))
		let capture = XCTAttachment(screenshot: app.screenshot())
		capture.name = "Global Pause toolbar after resuming"
		capture.lifetime = .keepAlways
		add(capture)
	}
}
