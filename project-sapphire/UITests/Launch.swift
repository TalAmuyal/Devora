import AppKit
import XCTest

/// Writes a global config in `dir` with one empty profile, and returns its URL: with none, the Workspace Hub a launch opens is the first-run welcome, which can't be closed.
/// `appearance` (`"light"` or `"dark"`) fixes the app's Appearance, for captures of a look the Mac isn't in.
func writeConfigWithAProfile(in dir: URL, appearance: String? = nil) throws -> URL {
	let profile = dir.appending(path: "Work")
	for subdirectory in ["repos", "workspaces"] {
		try FileManager.default.createDirectory(at: profile.appending(path: subdirectory), withIntermediateDirectories: true)
	}
	try #"{"name": "Work"}"#.write(to: profile.appending(path: "config.json"), atomically: true, encoding: .utf8)
	let config = dir.appending(path: "config.json")
	let appearanceEntry = appearance.map { #""appearance": "\#($0)","# } ?? ""
	try #"{\#(appearanceEntry)"repo-update":{"enabled":false},"profiles": ["\#(profile.path)"]}"#.write(to: config, atomically: true, encoding: .utf8)
	return config
}

/// Launches the app over the fake CLI with a profile and a scratch state of its own, creates the agent "Echo" from the
/// Agent Hub, and returns the app and the agent's prompt editor. Quitting in teardown reaps the fake agent before the
/// scratch directory goes.
@MainActor
func launchWithANewAgent(_ test: XCTestCase, scratchName: String) throws -> (XCUIApplication, XCUIElement) {
	let scratch = FileManager.default.temporaryDirectory.appending(path: "\(scratchName)-\(UUID().uuidString)")
	try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
	let config = try writeConfigWithAProfile(in: scratch)
	let app = XCUIApplication()
	app.launchEnvironment["DEVORA_TEST_MODE"] = "1"
	app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
	app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
	for key in ["DEVORA_TEST_CLAUDE_PROGRAM", "DEVORA_FAKE_FIXTURES"] {
		app.launchEnvironment[key] = try XCTUnwrap(ProcessInfo.processInfo.environment[key])
	}
	app.launchEnvironment["DEVORA_FAKE_REPORT"] = scratch.appending(path: "fake-report.jsonl").path
	app.launch()
	test.addTeardownBlock { @MainActor in
		if app.state != .notRunning {
			app.typeKey("q", modifierFlags: .command)
			XCTAssertTrue(app.wait(for: .notRunning, timeout: 10), "Quit must reap the fake agent")
		}
		if app.state == .notRunning { try? FileManager.default.removeItem(at: scratch) }
	}
	XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10), "The launch hub must appear")
	app.typeKey("s", modifierFlags: .control)
	XCTAssertTrue(app.buttons["+ New Agent"].waitForExistence(timeout: 10), "The Agent Hub must open")
	app.typeKey("n", modifierFlags: [])
	let name = app.textFields["new-agent-name"]
	XCTAssertTrue(name.waitForExistence(timeout: 10), "The New Agent dialog must open")
	name.click()
	name.typeText("Echo")
	app.buttons["Create"].click()
	let editor = app.textViews["agent-prompt"]
	XCTAssertTrue(editor.waitForExistence(timeout: 15), "Creating an agent must open its editor")
	return (app, editor)
}

extension XCUIApplication {
	/// Has the app take two lone Shift taps as a double tap however far apart they arrive (`DEVORA_TEST_SHIFT_TAP_WINDOW`), for a test that taps Shift-Shift; set before launching.
	/// XCUITest's synthesized taps can arrive seconds apart on a busy machine, past the 0.5 s a keyboard's double tap gets, which `ShiftDoubleTapTests` check on exact timestamps.
	@MainActor
	func allowSlowShiftDoubleTaps() {
		launchEnvironment["DEVORA_TEST_SHIFT_TAP_WINDOW"] = "60"
	}

	/// The Workspace Hub, which a launch with no tabs to restore opens.
	@MainActor
	var hubHeader: XCUIElement {
		descendants(matching: .any)["hub-header"].firstMatch
	}

	/// From a launch on a fresh app state, which opens the Workspace Hub: closes it with Escape and opens a shell tab with `+`, as a user would.
	@MainActor
	func openFirstShell() {
		XCTAssertTrue(hubHeader.waitForExistence(timeout: 10), "a launch with no tabs to restore opens the Workspace Hub")
		typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(hubHeader.waitForNonExistence(timeout: 5))
		openShell()
	}

	/// Opens a shell tab with `+` and waits for its label: from then on the tab is the active one, and its terminal has the keys, holding what is typed until its shell runs.
	@MainActor
	func openShell() {
		let shellTabs = staticTexts.matching(identifier: "Shell")
		let count = shellTabs.count
		buttons["New Shell"].click()
		XCTAssertTrue(shellTabs.element(boundBy: count).waitForExistence(timeout: 10), "+ opens a shell tab")
	}
}


/// Wheel scrolls that go where they're aimed. XCUITest posts a scroll's events at the pointer's position, and its move to
/// the target can land after the first of them, or after all of them on a busy machine, which then go to wherever the
/// pointer was. So the pointer goes there first, and the scroll waits until the system reports it there.
extension XCUICoordinate {
	@MainActor
	func scrollWithPointer(byDeltaX deltaX: CGFloat, deltaY: CGFloat, file: StaticString = #filePath, line: UInt = #line) {
		hover()
		let target = screenPoint
		awaitPointer(in: CGRect(x: target.x - 1, y: target.y - 1, width: 2, height: 2), what: "\(target)", file: file, line: line)
		scroll(byDeltaX: deltaX, deltaY: deltaY)
	}
}

extension XCUIElement {
	/// Scrolls at XCUITest's own point for the element, the one `hover()` moves to, once the pointer is there.
	@MainActor
	func scrollWithPointer(byDeltaX deltaX: CGFloat, deltaY: CGFloat, file: StaticString = #filePath, line: UInt = #line) {
		hover()
		awaitPointer(in: frame, what: "\(frame)", file: file, line: line)
		scroll(byDeltaX: deltaX, deltaY: deltaY)
	}
}

/// Waits until the pointer is in `area` (screen points, origin top-left, as XCUITest's) and has stopped moving.
@MainActor
private func awaitPointer(in area: CGRect, what: String, file: StaticString, line: UInt) {
	func pointer() -> CGPoint {
		let location = NSEvent.mouseLocation  // origin bottom-left of the main screen
		return CGPoint(x: location.x, y: (NSScreen.screens.first?.frame.height ?? 0) - location.y)
	}
	var last = pointer()
	let deadline = Date().addingTimeInterval(10)
	while Date() < deadline {
		RunLoop.current.run(until: Date().addingTimeInterval(0.05))
		let now = pointer()
		if area.contains(now) && now == last { return }
		last = now
	}
	XCTFail("The pointer must reach \(what) before a scroll there; it is at \(last)", file: file, line: line)
}

extension XCUIApplication {
	/// Scroll the detail's margin so a nested prompt preview cannot consume the wheel events.
	@MainActor
	func scrollSettingsDetail(to control: XCUIElement) {
		let detail = scrollViews["settings-detail-scroll"]
		for _ in 0..<30 {
			let visible = detail.frame.insetBy(dx: 0, dy: 40)
			if visible.contains(control.frame) { return }
			let delta: CGFloat = control.frame.midY > visible.midY ? -200 : 200
			detail.coordinate(withNormalizedOffset: CGVector(dx: 0.01, dy: 0.5))
				.scrollWithPointer(byDeltaX: 0, deltaY: delta)
		}
		XCTFail("Settings control did not scroll into view: \(control.identifier)")
	}
}
