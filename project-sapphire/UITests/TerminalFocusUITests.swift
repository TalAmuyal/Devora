import XCTest

/// Real key events, through AppKit's responder chain and libghostty's input path, reach the shell of the tab the user is looking at.
final class TerminalFocusUITests: XCTestCase {
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
	func testTypedKeysFollowTheActiveTab() throws {
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": []}"#.write(to: config, atomically: true, encoding: .utf8)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		// The shells inherit it, which keeps the typed commands short.
		app.launchEnvironment["UITEST_OUT"] = scratch.path
		app.launch()

		// Each tab's terminal has its own id, which its shell sees as DEVORA_PTY_ID.
		let first = try typeTerminalID(into: app, file: "first")
		app.buttons["New Shell"].click()
		let second = try typeTerminalID(into: app, file: "second")
		XCTAssertNotEqual(first, second, "the new tab's terminal should have the keys")

		app.staticTexts.matching(identifier: "Shell").element(boundBy: 0).click()
		XCTAssertEqual(try typeTerminalID(into: app, file: "back-to-first"), first, "the first tab's terminal should have the keys again")
	}

	/// Types a command that writes the focused terminal's id to a file, and reads it back.
	@MainActor
	private func typeTerminalID(into app: XCUIApplication, file: String) throws -> String {
		app.typeText("echo $DEVORA_PTY_ID >$UITEST_OUT/\(file)\n")
		let url = scratch.appending(path: file)
		let deadline = Date.now.addingTimeInterval(15)
		while Date.now < deadline {
			if let id = try? String(contentsOf: url, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines), !id.isEmpty {
				return id
			}
			Thread.sleep(forTimeInterval: 0.1)
		}
		XCTFail("no shell ran the command typed for \(file)")
		return ""
	}
}
