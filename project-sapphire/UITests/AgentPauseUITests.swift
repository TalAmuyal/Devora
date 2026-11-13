import XCTest

final class AgentPauseUITests: XCTestCase {
	@MainActor
	func testPauseStopAndKeepPausedThroughGlobalResume() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-pause-widget-\(UUID().uuidString)")
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
				XCTAssertTrue(app.wait(for: .notRunning, timeout: 10), "Quit must reap the fake agent")
			}
			if app.state == .notRunning { try? FileManager.default.removeItem(at: scratch) }
		}
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		app.typeKey("s", modifierFlags: .control)
		XCTAssertTrue(app.buttons["+ New Agent"].waitForExistence(timeout: 10))
		app.typeKey("n", modifierFlags: [])
		let name = app.textFields["new-agent-name"]
		XCTAssertTrue(name.waitForExistence(timeout: 10))
		name.click()
		name.typeText("Echo")
		app.buttons["Create"].click()
		let editor = app.textViews["agent-prompt"]
		XCTAssertTrue(editor.waitForExistence(timeout: 15))
		let pause = app.buttons["agent-pause-control"]
		waitFor("The running agent exposes Pause") { pause.exists && pause.isEnabled && pause.label == "Pause agent" }

		func sendPermissionTurn() {
			editor.click()
			editor.typeText("permission")
			waitFor("The draft is ready") { app.buttons["Send"].isEnabled }
			app.buttons["Send"].click()
			XCTAssertTrue(app.buttons["Allow ⌘Y"].waitForExistence(timeout: 10))
			XCTAssertTrue(app.buttons["agent-stop"].exists)
			XCTAssertFalse(app.buttons["Send"].exists)
		}

		sendPermissionTurn()
		pause.click()
		waitFor("Pause waits for the permission-held turn") { pause.label == "Pausing after this turn" && !pause.isEnabled }
		XCTAssertTrue(app.buttons["agent-stop"].isEnabled)
		app.buttons["Allow ⌘Y"].click()
		waitFor("The completed turn exposes Resume") { pause.label == "Resume agent" && pause.isEnabled }
		XCTAssertFalse(app.buttons["agent-stop"].exists)
		pause.click()
		waitFor("Resume exposes Pause") { pause.label == "Pause agent" && pause.isEnabled }

		editor.click()
		editor.typeText("stream")
		waitFor("The streaming draft is ready") { app.buttons["Send"].isEnabled }
		app.buttons["Send"].click()
		XCTAssertTrue(app.buttons["agent-stop"].waitForExistence(timeout: 10))
		app.buttons["agent-stop"].click()
		XCTAssertTrue(app.buttons["agent-stop"].waitForNonExistence(timeout: 10))
		XCTAssertTrue(app.buttons["Send"].exists)
		XCTAssertTrue(app.buttons["Allow ⌘Y"].waitForNonExistence(timeout: 10))

		sendPermissionTurn()
		let window = app.windows.firstMatch
		let corner = window.coordinate(withNormalizedOffset: CGVector(dx: 1, dy: 1)).withOffset(CGVector(dx: -2, dy: -2))
		corner.click(forDuration: 0.1, thenDragTo: corner.withOffset(CGVector(dx: -2000, dy: 0)))
		app.buttons["global-pause-toggle"].click()
		let global = app.descendants(matching: .any)["agent-global-pause-status"].firstMatch
		waitFor("The widget shows the draining global hold") {
			global.exists && (global.value as? String) == "Pausing after this turn"
		}
		let keep = app.buttons["agent-keep-paused"]
		XCTAssertTrue(keep.isHittable)
		XCTAssertTrue(app.buttons["agent-stop"].isHittable)
		XCTAssertLessThanOrEqual(app.buttons["agent-stop"].frame.maxX, window.frame.maxX)
		keep.click()
		waitFor("Keep paused becomes selected") { keep.isSelected && !keep.isEnabled }
		XCTAssertTrue((keep.value as? String)?.contains("On.") == true)
		app.buttons["Allow ⌘Y"].click()
		XCTAssertTrue(app.buttons["agent-stop"].waitForNonExistence(timeout: 10))
		XCTAssertFalse(pause.exists, "An individual Resume action is unavailable while globally held")
		app.buttons["global-pause-toggle"].click()
		waitFor("Releasing Devora leaves this agent individually paused") {
			pause.exists && pause.label == "Resume agent" && pause.isEnabled
		}
		XCTAssertTrue(pause.isHittable)
		let capture = XCTAttachment(screenshot: window.screenshot())
		capture.name = "Kept paused after global resume at minimum width"
		capture.lifetime = .keepAlways
		add(capture)
		pause.click()
		waitFor("Individual Resume starts the agent") { pause.label == "Pause agent" && pause.isEnabled }
	}

	@MainActor
	private func waitFor(_ description: String, _ condition: @escaping () -> Bool) {
		let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 15), .completed, description)
	}
}
