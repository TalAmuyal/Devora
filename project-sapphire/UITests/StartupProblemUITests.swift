import XCTest

/// An app state that can't be opened stops the launch until the user answers, with real clicks: Quit leaves the state as it was; Start Fresh keeps it aside and goes on with the launch.
final class StartupProblemUITests: XCTestCase {
	private var scratch: URL!

	override func setUpWithError() throws {
		continueAfterFailure = false
		scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-uitest-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch.appending(path: "state"), withIntermediateDirectories: true)
	}

	override func tearDownWithError() throws {
		try? FileManager.default.removeItem(at: scratch)
	}

	@MainActor
	func testAnUnreadableStateIsLeftAloneOnQuitAndKeptAsideOnStartFresh() throws {
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": []}"#.write(to: config, atomically: true, encoding: .utf8)
		let stateDir = scratch.appending(path: "state")
		let corrupt = Data(repeating: 0x5a, count: 8192)
		try corrupt.write(to: stateDir.appending(path: "state.db"))
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = stateDir.path

		app.launch()
		XCTAssertTrue(app.staticTexts["The saved tabs couldn't be opened"].waitForExistence(timeout: 10))
		app.buttons["Quit"].click()
		XCTAssertTrue(app.wait(for: .notRunning, timeout: 10), "Quit should quit")
		XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: stateDir.path), ["state.db"])
		XCTAssertEqual(try Data(contentsOf: stateDir.appending(path: "state.db")), corrupt)

		app.launch()
		app.buttons["Start Fresh"].click()
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10), "a fresh state has no tabs, so the launch goes on to the Workspace Hub")
		let kept = try FileManager.default.contentsOfDirectory(atPath: stateDir.path).filter { $0.hasPrefix("state-unreadable-") }
		XCTAssertEqual(kept.count, 1)
		XCTAssertEqual(try Data(contentsOf: stateDir.appending(path: kept[0])), corrupt)
	}
}
