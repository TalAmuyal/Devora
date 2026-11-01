import XCTest

/// The Settings Hub with real input: typing into a settings card's input, where Enter commits and the list's keys (`j`, `d`) type rather than act, none of which the test bridge can drive.
final class SettingsHubUITests: XCTestCase {
	private var scratch: URL!
	private var app: XCUIApplication!

	override func setUpWithError() throws {
		continueAfterFailure = false
		scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-uitest-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
	}

	override func tearDownWithError() throws {
		try? FileManager.default.removeItem(at: scratch)
	}

	@MainActor
	func testTypingIntoACardInputCommitsOnEnterAndNeverReachesTheList() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))

		app.typeKey("p", modifierFlags: .shift)
		let settings = element("settings-hub")
		XCTAssertTrue(settings.waitForExistence(timeout: 5), "P opens the Settings Hub")
		let segments = element("config-segments-terminal.default-app")
		XCTAssertTrue(segments.waitForExistence(timeout: 5), "the active profile's cards load")
		segments.radioButtons["Set"].click()

		let input = element("config-input-terminal.default-app")
		XCTAssertTrue(input.waitForExistence(timeout: 5), "Set reveals the input")
		XCTAssertTrue(waitFor(input, "hasKeyboardFocus == true"), "and gives it the focus")
		app.typeText("jdk")
		app.typeKey(.return, modifierFlags: [])

		let profileConfig = scratch.appending(path: "Work/config.json")
		XCTAssertTrue(
			waitFor(input, "value == 'jdk'"), "j and d typed into the input stay in it")
		XCTAssertFalse(element("confirmation-dialog").exists, "d typed into the input doesn't delete the profile")
		let stored = XCTNSPredicateExpectation(
			predicate: NSPredicate { _, _ in
				(try? String(contentsOf: profileConfig, encoding: .utf8))?.contains(#""default-app": "jdk""#) == true
			}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [stored], timeout: 5), .completed, "Enter commits the value to the profile's config")
		XCTAssertTrue(input.exists, "the list's cursor stayed on the profile")

		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(waitFor(input, "hasKeyboardFocus == false"), "Escape leaves the input")
		XCTAssertTrue(settings.exists, "and doesn't close the Settings Hub")
		app.typeKey("j", modifierFlags: [])
		XCTAssertTrue(element("profile-form-name").waitForExistence(timeout: 5), "out of the input, j moves to the New Profile row")
	}

	@MainActor
	private func waitFor(_ element: XCUIElement, _ predicate: String) -> Bool {
		XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: predicate), object: element)], timeout: 5) == .completed
	}

	@MainActor
	private func element(_ identifier: String) -> XCUIElement {
		app.descendants(matching: .any)[identifier].firstMatch
	}
}
