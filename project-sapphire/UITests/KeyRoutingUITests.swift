import CoreGraphics
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

	/// What the app-wide shortcuts made of each modifier event (`InputTrace`).
	private var inputTrace: URL { scratch.appending(path: "input-trace.log") }

	override func tearDownWithError() throws {
		try? FileManager.default.removeItem(at: scratch)
	}

	/// Launches the app on a fresh config and app state, and opens one shell tab; the shells see `UITEST_OUT`.
	@MainActor
	private func launch() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		app = XCUIApplication()
		app.allowSlowShiftDoubleTaps()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launchEnvironment["UITEST_OUT"] = scratch.path
		app.launchEnvironment["DEVORA_INPUT_TRACE"] = inputTrace.path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		app.openFirstShell()
	}

	@MainActor
	func testCtrlArrowsSwitchTabsAndTheFocusFollows() throws {
		try launch()
		let first = try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/first", awaiting: "first")
		app.openShell()
		let second = try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/second", awaiting: "second")
		XCTAssertNotEqual(first, second)

		app.typeKey(.rightArrow, modifierFlags: .control)
		XCTAssertEqual(try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/wrapped", awaiting: "wrapped"), first, "Ctrl+Right from the last tab wraps to the first, and its terminal has the keys")

		app.typeKey(.leftArrow, modifierFlags: .control)
		XCTAssertEqual(try runInShell("echo $DEVORA_PTY_ID >$UITEST_OUT/back", awaiting: "back"), second)
	}

	@MainActor
	func testCtrlCInterruptsTheForegroundProcess() throws {
		try launch()
		// `/bin/sh` rather than the login shell, whose own handling of an interrupted command list differs between shells.
		_ = try runInShell(
			#"sh -c 'trap "echo interrupted >$UITEST_OUT/interrupted; exit" INT; echo started >$UITEST_OUT/started; while :; do sleep 0.1; done'"#,
			awaiting: "started")
		app.typeKey("c", modifierFlags: .control)
		XCTAssertEqual(try waitForFile("interrupted"), "interrupted")
	}

	@MainActor
	func testOptionComposedTextReachesTheShell() throws {
		try launch()
		// Option-e then e is the dead-key accent of the US layouts: the input method composes it into one character.
		app.typeText("printf %s ")
		app.typeKey("e", modifierFlags: .option)
		app.typeKey("e", modifierFlags: [])
		XCTAssertEqual(try runInShell(" >$UITEST_OUT/composed", awaiting: "composed"), "é")
	}

	@MainActor
	func testShiftShiftOpensAndReopensThePaletteWithRealModifierEvents() throws {
		try launch()
		func tapShift() {
			XCUIElement.perform(withKeyModifiers: .shift) {}
		}
		let field = app.descendants(matching: .any)["command-palette-search"].firstMatch
		tapShift(); tapShift()
		app.typeText("q")
		assertShiftShiftOpened(field)
		XCTAssertEqual(field.value as? String, "q")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(field.waitForNonExistence(timeout: 5))
		tapShift(); tapShift()
		assertShiftShiftOpened(field)
	}

	/// Fails unless the palette's field appears. On failure, it attaches what the app made of the taps: whether they arrived, for which window, and how the double tap read them.
	/// Beside that is whether the system still has Shift down. If it does, XCUITest never posted the last release, which it has done right after a relaunch.
	/// If not, the release was posted, or an earlier one is the one missing.
	@MainActor
	private func assertShiftShiftOpened(_ field: XCUIElement, file: StaticString = #filePath, line: UInt = #line) {
		if field.waitForExistence(timeout: 5) { return }
		let appTrace = (try? String(contentsOf: inputTrace, encoding: .utf8)) ?? "(no input trace)"
		let shiftDown = CGEventSource.flagsState(.combinedSessionState).contains(.maskShift)
		let trace = XCTAttachment(string: "\(appTrace)system now: Shift down \(shiftDown)\n")
		trace.name = "The app's shortcut decisions"
		trace.lifetime = .keepAlways
		add(trace)
		XCTFail("Shift-Shift must open the command palette\n\(app.debugDescription)", file: file, line: line)
	}

	/// Types a command and Return, then waits for the file it writes in `UITEST_OUT` and returns its contents.
	@MainActor
	private func runInShell(_ command: String, awaiting file: String) throws -> String {
		app.typeText(command + "\n")
		return try waitForFile(file)
	}

	/// Waits for a file in `UITEST_OUT` to have contents, and returns them.
	private func waitForFile(_ file: String) throws -> String {
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
