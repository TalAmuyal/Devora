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
		XCTAssertTrue(search.waitForExistence(timeout: 5))
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
