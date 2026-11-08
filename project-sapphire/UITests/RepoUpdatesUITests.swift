import XCTest

final class RepoUpdatesUITests: XCTestCase {
	@MainActor
	func testUserDefaultsEditsTheUpdaterAndThePaletteRunsIt() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-repo-updates-\(UUID().uuidString)")
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
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		app.typeKey("p", modifierFlags: .shift)
		app.staticTexts["User Defaults"].firstMatch.click()
		let enabled = app.descendants(matching: .any)["config-segments-repo-update.enabled"].firstMatch
		XCTAssertTrue(enabled.waitForExistence(timeout: 5))
		app.scrollSettingsDetail(to: enabled)
		enabled.radioButtons["Off"].click()
		let interval = app.descendants(matching: .any)["config-segments-repo-update.interval-minutes"].firstMatch
		app.scrollSettingsDetail(to: interval)
		interval.radioButtons["Set"].click()
		let input = app.textFields["config-input-repo-update.interval-minutes"]
		XCTAssertTrue(input.waitForExistence(timeout: 5))
		input.click()
		app.typeText("15")
		app.typeKey(.return, modifierFlags: [])
		let saved = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
			guard let data = try? Data(contentsOf: config),
			      let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
			      let settings = json["repo-update"] as? [String: Any] else { return false }
			return settings["enabled"] as? Bool == false && settings["interval-minutes"] as? Int == 15
		}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [saved], timeout: 5), .completed)
		let settingsCapture = XCTAttachment(screenshot: app.windows.firstMatch.screenshot())
		settingsCapture.name = "Repository Updates User Defaults"
		settingsCapture.lifetime = .keepAlways
		add(settingsCapture)
		app.typeKey(.escape, modifierFlags: [])
		app.typeKey(.escape, modifierFlags: [])
		app.openFirstShell()
		XCUIElement.perform(withKeyModifiers: .shift) {}
		XCUIElement.perform(withKeyModifiers: .shift) {}
		let search = app.descendants(matching: .any)["command-palette-search"].firstMatch
		XCTAssertTrue(search.waitForExistence(timeout: 5))
		app.typeText("Run Repo Updates")
		let capture = XCTAttachment(screenshot: app.windows.firstMatch.screenshot())
		capture.name = "Repository Updates palette action"
		capture.lifetime = .keepAlways
		add(capture)
		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(search.waitForNonExistence(timeout: 5))
	}
}
