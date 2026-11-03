import XCTest

/// F1 opens the real guide over a dialog and dismissal returns to it.
final class UserGuideUITests: XCTestCase {
	@MainActor
	func testGuideOverDialogWithRealKeys() throws {
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-guide-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		defer { try? FileManager.default.removeItem(at: scratch) }
		let config = try writeConfigWithAProfile(in: scratch)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_TEST_MODE"] = "1"
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		defer { app.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		app.typeText("n")
		let dialog = app.descendants(matching: .any)["new-task-dialog"].firstMatch
		XCTAssertTrue(dialog.waitForExistence(timeout: 5))
		app.typeKey(.F1, modifierFlags: [])
		let guide = app.descendants(matching: .any)["user-guide"].firstMatch
		XCTAssertTrue(guide.waitForExistence(timeout: 5))
		let body = app.webViews.firstMatch.staticTexts.matching(NSPredicate(format: "value CONTAINS %@", "Work in Devora")).firstMatch
		let rendered = body.waitForExistence(timeout: 15)
		if !rendered {
			let capture = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
			capture.lifetime = .keepAlways
			add(capture)
		}
		XCTAssertTrue(rendered, "the actual Markdown content renders")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(guide.waitForNonExistence(timeout: 5))
		XCTAssertTrue(dialog.exists)
	}
}
