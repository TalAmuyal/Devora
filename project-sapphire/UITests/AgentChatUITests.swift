import XCTest

/// Real AppKit input over the fake CLI and shared WKWebView transcript.
final class AgentChatUITests: XCTestCase {
	@MainActor
	func testMultilineSendPermissionKeysAndDraftRestore() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(
			path: "sapphire-chat-uitest-\(UUID().uuidString)")
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
		addTeardownBlock { @MainActor in
			if app.state != .notRunning {
				app.typeKey("q", modifierFlags: .command)
				XCTAssertTrue(
					app.wait(for: .notRunning, timeout: 10),
					"Quit must reap the fake agent before fixture removal")
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
		editor.click()
		editor.typeText("first")
		editor.typeKey(.return, modifierFlags: [])
		editor.typeKey(.return, modifierFlags: .shift)
		XCTAssertEqual(editor.value as? String, "first\n\n")
		let ready = NSPredicate { _, _ in app.buttons["Send"].isEnabled }
		// The editor is usable while the agent's CLI is still starting.
		let started = XCTNSPredicateExpectation(predicate: ready, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [started], timeout: 15), .completed,
			"The agent must be ready to send the first draft. \(app.debugDescription)")
		editor.typeKey(.return, modifierFlags: .command)
		let cleared = NSPredicate(format: "value == %@", "")
		expectation(for: cleared, evaluatedWith: editor)
		waitForExpectations(timeout: 10)
		editor.typeText("permission")
		// Draft acknowledgement can arrive before the preceding turn finishes.
		let finished = XCTNSPredicateExpectation(predicate: ready, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [finished], timeout: 15), .completed,
			"The preceding turn must finish before the permission prompt is sent. \(app.debugDescription)")
		app.typeKey(.return, modifierFlags: .control)
		XCTAssertTrue(app.buttons["Allow ⌘Y"].waitForExistence(timeout: 10), "The permission turn must request approval")
		editor.click()
		editor.typeKey("y", modifierFlags: .command)
		XCTAssertTrue(app.buttons["Allow ⌘Y"].waitForNonExistence(timeout: 10), "Command+Y must answer the permission")
		editor.typeText("keep this unsent")
		editor.typeKey(.escape, modifierFlags: [])
		XCTAssertEqual(editor.value as? String, "keep this unsent")
		app.typeKey("q", modifierFlags: .command)
		XCTAssertTrue(app.wait(for: .notRunning, timeout: 10), "Quit must flush the draft and stop the agent")
		app.launch()
		XCTAssertTrue(editor.waitForExistence(timeout: 15), "Relaunch must restore the agent tab")
		XCTAssertEqual(editor.value as? String, "keep this unsent")
	}
}
