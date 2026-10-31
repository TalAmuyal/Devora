import XCTest

/// The first-run welcome and its profile form with real input: the keys that move through the form and submit it, the hub refusing to close, and the folder panel behind "Browse…", none of which the test bridge can drive.
final class ProfileUITests: XCTestCase {
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

	/// Launches the app with no profiles, so the Workspace Hub it opens is the first-run welcome.
	@MainActor
	private func launchWithNoProfiles() throws -> XCUIElement {
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": []}"#.write(to: config, atomically: true, encoding: .utf8)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		let welcome = element("first-run-welcome")
		XCTAssertTrue(welcome.waitForExistence(timeout: 10), "a launch with no profiles opens on the first-run welcome")
		return welcome
	}

	@MainActor
	func testTheWelcomeCantBeClosedAndTabAndEnterFillInAndSubmitItsForm() throws {
		let welcome = try launchWithNoProfiles()
		app.typeKey(.escape, modifierFlags: [])
		app.typeKey("q", modifierFlags: [])
		app.typeKey("s", modifierFlags: .control)
		XCTAssertFalse(welcome.waitForNonExistence(timeout: 1), "the welcome refuses Escape and q, and Ctrl+S never closes the hub")

		app.typeKey(.tab, modifierFlags: [])
		app.typeText("Mine")
		XCTAssertEqual(element("profile-form-name").value as? String, "Mine", "Tab moves the focus to the name")
		app.typeKey(.tab, modifierFlags: [])
		let root = scratch.appending(path: "mine").path
		app.typeKey("a", modifierFlags: .command)
		app.typeText(root)
		XCTAssertEqual(element("profile-form-path").value as? String, root, "Tab moves the focus on to the root path")
		XCTAssertTrue(waitForStatus("✓ New profile — the structure below will be created"))

		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(welcome.waitForNonExistence(timeout: 10), "Enter registers the profile")
		XCTAssertTrue(FileManager.default.fileExists(atPath: "\(root)/workspaces"))
		XCTAssertTrue(element("hub-profile-dropdown").waitForExistence(timeout: 5))
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(app.hubHeader.waitForNonExistence(timeout: 5), "with a profile, the hub closes")
	}

	@MainActor
	func testBrowseFillsInTheRootPathFromTheFolderPanel() throws {
		let legacy = scratch.appending(path: "Legacy")
		try FileManager.default.createDirectory(at: legacy, withIntermediateDirectories: true)
		try #"{"name": "Legacy"}"#.write(to: legacy.appending(path: "config.json"), atomically: true, encoding: .utf8)
		_ = try launchWithNoProfiles()

		element("profile-form-browse").click()
		let panel = app.sheets.firstMatch
		XCTAssertTrue(panel.waitForExistence(timeout: 10), "Browse… opens the folder panel")
		// Go to Folder, in the panel.
		app.typeKey("g", modifierFlags: [.command, .shift])
		app.typeText(legacy.path + "\n")
		panel.buttons["Choose"].click()

		XCTAssertTrue(panel.waitForNonExistence(timeout: 5))
		XCTAssertEqual(element("profile-form-path").value as? String, legacy.path)
		XCTAssertTrue(waitForStatus("◆ Existing profile \"Legacy\" found — it will be registered as-is"))
		XCTAssertEqual(element("profile-form-name").value as? String, "Legacy", "an existing profile's name replaces the one typed")
	}

	/// Waits for the form's validation status to read `text`.
	@MainActor
	private func waitForStatus(_ text: String) -> Bool {
		let shown = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", text), object: element("profile-form-status"))
		return XCTWaiter.wait(for: [shown], timeout: 5) == .completed
	}

	@MainActor
	private func element(_ identifier: String) -> XCUIElement {
		app.descendants(matching: .any)[identifier].firstMatch
	}
}
