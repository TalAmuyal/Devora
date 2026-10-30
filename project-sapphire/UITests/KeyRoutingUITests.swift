import XCTest

/// Real key events through the key router: Sapphire's own chords act on the app, and every other key reaches the shell unchanged.
final class KeyRoutingUITests: XCTestCase {
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

	/// Launches the app on a fresh config and app state, so it starts with one shell tab; the shells see `UITEST_OUT`.
	@MainActor
	private func launch() throws {
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": []}"#.write(to: config, atomically: true, encoding: .utf8)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launchEnvironment["UITEST_OUT"] = scratch.path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
	}

	@MainActor
	func testCtrlArrowsSwitchTabsAndTheFocusFollows() throws {
		try launch()
		let first = try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/first", awaiting: "first")
		app.buttons["New Shell"].click()
		let second = try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/second", awaiting: "second")
		XCTAssertNotEqual(first, second)

		app.typeKey(.rightArrow, modifierFlags: .control)
		XCTAssertEqual(try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/wrapped", awaiting: "wrapped"), first, "Ctrl+Right from the last tab wraps to the first, and its terminal has the keys")

		app.typeKey(.leftArrow, modifierFlags: .control)
		XCTAssertEqual(try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/back", awaiting: "back"), second)
	}

	@MainActor
	func testCtrlCReachesTheShell() throws {
		try launch()
		_ = try runInShell("echo started >$UITEST_OUT/started; sleep 60; echo late >$UITEST_OUT/late", awaiting: "started")
		app.typeKey("c", modifierFlags: .control)
		// Within the sleep: only an interrupt lets the shell run the next command this soon.
		_ = try runInShell("echo after >$UITEST_OUT/after", awaiting: "after")
		XCTAssertFalse(FileManager.default.fileExists(atPath: scratch.appending(path: "late").path))
	}

	@MainActor
	func testOptionComposedTextReachesTheShell() throws {
		try launch()
		// Option-e then e is the dead-key accent of the US layouts: the input method composes it into one character.
		app.typeText("printf %s ")
		app.typeKey("e", modifierFlags: .option)
		app.typeKey("e", modifierFlags: [])
		let composed = try runInShell(" >$UITEST_OUT/composed", awaiting: "composed")
		XCTAssertEqual(composed, "é", "got the scalars \(composed.unicodeScalars.map { String($0.value, radix: 16) })")
	}

	/// Types a command and Return, then waits for the file it writes in `UITEST_OUT` and returns its contents.
	@MainActor
	private func runInShell(_ command: String, awaiting file: String) throws -> String {
		app.typeText(command + "\n")
		let url = scratch.appending(path: file)
		let deadline = Date.now.addingTimeInterval(15)
		while Date.now < deadline {
			if let text = try? String(contentsOf: url, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty {
				return text
			}
			Thread.sleep(forTimeInterval: 0.1)
		}
		XCTFail("the shell didn't write \(file)")
		return ""
	}
}
