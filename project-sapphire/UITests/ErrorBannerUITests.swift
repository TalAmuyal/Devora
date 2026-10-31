import XCTest

/// An error banner goes with a real click on its ×, which the acceptance scenarios' bridge can't make.
final class ErrorBannerUITests: XCTestCase {
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
	func testClickingTheCrossDismissesTheBanner() throws {
		let config = scratch.appending(path: "config.json")
		try "{not json".write(to: config, atomically: true, encoding: .utf8)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path

		app.launch()
		let banner = app.descendants(matching: .any).matching(identifier: "error-banner").firstMatch
		XCTAssertTrue(banner.waitForExistence(timeout: 10), "the launch's list_profiles should fail on the config")
		let failure = NSPredicate(format: "label BEGINSWITH %@ OR value BEGINSWITH %@", "list_profiles failed", "list_profiles failed")
		XCTAssertTrue(banner.descendants(matching: .any).matching(failure).firstMatch.exists, banner.debugDescription)

		banner.buttons["Dismiss error"].click()
		XCTAssertTrue(banner.waitForNonExistence(timeout: 5))
		app.terminate()
	}
}
