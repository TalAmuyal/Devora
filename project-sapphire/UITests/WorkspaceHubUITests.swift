import XCTest

/// The Workspace Hub with real input: what it draws as it opens, and typing in its filter field, which the test bridge's key-downs can't vouch for.
final class WorkspaceHubUITests: XCTestCase {
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

	/// Launches the app on a profile with two active workspaces, "Fix alpha" (`ws-1`) and "Fix beta" (`ws-2`).
	@MainActor
	private func launch() throws {
		let profile = scratch.appending(path: "Work")
		try FileManager.default.createDirectory(at: profile.appending(path: "repos"), withIntermediateDirectories: true)
		try #"{"name": "Work"}"#.write(to: profile.appending(path: "config.json"), atomically: true, encoding: .utf8)
		for (id, title) in [("ws-1", "Fix alpha"), ("ws-2", "Fix beta")] {
			let workspace = profile.appending(path: "workspaces/\(id)")
			try FileManager.default.createDirectory(at: workspace, withIntermediateDirectories: true)
			try Data().write(to: workspace.appending(path: "initialized"))
			try #"{"title": "\#(title)"}"#.write(to: workspace.appending(path: "task.json"), atomically: true, encoding: .utf8)
		}
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": ["\#(profile.path)"]}"#.write(to: config, atomically: true, encoding: .utf8)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
	}

	/// A fresh app state has no tabs to restore, so the launch opens the hub.
	@MainActor
	func testTheHubDrawsItsHeaderFilterAndHintsAsItOpens() throws {
		try launch()
		for identifier in ["hub-header", "hub-filter", "hub-hint-bar"] {
			XCTAssertTrue(element(identifier).waitForExistence(timeout: 5), "\(identifier) isn't drawn")
		}
	}

	@MainActor
	func testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing() throws {
		try launch()
		XCTAssertTrue(app.staticTexts["Fix beta"].waitForExistence(timeout: 10))

		app.typeKey("f", modifierFlags: [])
		// `q` types into the field rather than closing the hub.
		app.typeText("beta q")
		XCTAssertEqual(element("hub-filter").value as? String, "beta q")
		XCTAssertTrue(app.staticTexts["No workspaces found"].waitForExistence(timeout: 5))
		app.typeKey(.delete, modifierFlags: [])
		app.typeKey(.delete, modifierFlags: [])
		XCTAssertTrue(app.staticTexts["Fix beta"].waitForExistence(timeout: 5))
		XCTAssertFalse(app.staticTexts["Fix alpha"].exists)

		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(element("hub-header").exists, "the first Escape only leaves the filter")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(element("hub-header").waitForNonExistence(timeout: 5))
	}

	@MainActor
	private func element(_ identifier: String) -> XCUIElement {
		app.descendants(matching: .any)[identifier].firstMatch
	}
}
