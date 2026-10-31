import XCTest

/// Closing the window with real input quits the app without closing its tabs: the terminals end with the window, and that must not read as their shells exiting.
final class WindowCloseUITests: XCTestCase {
	private var scratch: URL!

	override func setUpWithError() throws {
		continueAfterFailure = false
		scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-uitest-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
	}

	override func tearDownWithError() throws {
		try? FileManager.default.removeItem(at: scratch)
	}

	@MainActor
	func testClosingTheWindowQuitsAndKeepsTheTabs() throws {
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": []}"#.write(to: config, atomically: true, encoding: .utf8)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		app.openFirstShell()
		app.openShell()
		let shellTabs = app.staticTexts.matching(identifier: "Shell")

		app.typeKey("w", modifierFlags: .command)
		XCTAssertTrue(app.wait(for: .notRunning, timeout: 10), "closing the window should quit")

		app.launch()
		XCTAssertTrue(shellTabs.element(boundBy: 1).waitForExistence(timeout: 10), "both tabs should be back")
		XCTAssertEqual(shellTabs.count, 2)
		XCTAssertFalse(app.hubHeader.exists, "restored tabs show without the Workspace Hub")
	}
}
