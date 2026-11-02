import XCTest

/// Opens Health and re-runs it with actual keyboard input.
final class HealthHubUITests: XCTestCase {
	private var scratch: URL!
	private var app: XCUIApplication!

	override func setUpWithError() throws {
		continueAfterFailure = false
		scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-health-uitest-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
	}

	override func tearDownWithError() throws {
		try? FileManager.default.removeItem(at: scratch)
	}

	@MainActor
	func testHealthOpensAndRerunsWithRealKeys() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		let fixture = scratch.appending(path: "health-report.json")
		let json = try XCTUnwrap(ProcessInfo.processInfo.environment["DEVORA_HEALTH_FIXTURE_JSON"])
		try json.write(to: fixture, atomically: true, encoding: .utf8)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_TEST_MODE"] = "1"
		app.launchEnvironment["DEVORA_TEST_HEALTH_FIXTURE"] = fixture.path
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		app.typeKey("h", modifierFlags: .shift)
		XCTAssertTrue(app.staticTexts["0.0.0-test"].waitForExistence(timeout: 10), "H opens Health and renders the report")
		try json.replacingOccurrences(of: "0.0.0-test", with: "0.0.1-test").write(to: fixture, atomically: true, encoding: .utf8)
		app.typeKey("r", modifierFlags: [])
		XCTAssertTrue(app.staticTexts["0.0.1-test"].waitForExistence(timeout: 10), "r loads the updated report")
	}
}
