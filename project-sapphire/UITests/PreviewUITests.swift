import XCTest

final class PreviewUITests: XCTestCase {
	@MainActor
	func testDebiPreviewCloseReturnsKeyboardFocusToTerminal() throws {
		try exercisePreview(extension: "md", contents: "# Preview document\n", format: "Markdown")
	}

	@MainActor
	func testSandboxedHTMLCloseReturnsKeyboardFocusToTerminal() throws {
		try exercisePreview(extension: "html", contents: "<h1>Preview document</h1><script>document.querySelector('h1').remove()</script>", format: "HTML")
	}

	@MainActor
	func testDividerDragPersistsClampsAndStackedCloseKeepsTheTerminalAlive() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "preview-drag-\(UUID())")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		defer { try? FileManager.default.removeItem(at: scratch) }
		let config = try writeConfigWithAProfile(in: scratch)
		let first = scratch.appending(path: "first.md"), second = scratch.appending(path: "second.md")
		try "# First pane\n".write(to: first, atomically: true, encoding: .utf8)
		try "# Second pane\n".write(to: second, atomically: true, encoding: .utf8)
		let debi = try XCTUnwrap(ProcessInfo.processInfo.environment["SAPPHIRE_TEST_DEBI"])
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		defer { app.terminate() }
		app.openFirstShell()
		let before = scratch.appending(path: "pid-before"), after = scratch.appending(path: "pid-after")
		app.typeText("printf '%s' $$ >'\(before.path)'\n")
		waitUntil { FileManager.default.fileExists(atPath: before.path) }
		app.typeText("'\(debi)' preview '\(first.path)'\n")
		let divider = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "preview-divider-")).firstMatch
		XCTAssertTrue(divider.waitForExistence(timeout: 15))
		let window = app.windows.firstMatch
		window.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.1)).click()
		let original = divider.frame.midX - window.frame.minX
		let start = divider.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
		// The press API synthesizes touch; macOS needs the mouse click-and-drag API.
		start.click(forDuration: 0.1, thenDragTo: start.withOffset(CGVector(dx: -80, dy: 0)))
		let dragCapture = XCTAttachment(screenshot: app.screenshot())
		dragCapture.name = "Divider after real drag"
		dragCapture.lifetime = .keepAlways
		add(dragCapture)
		waitUntil { divider.frame.midX - window.frame.minX < original - 40 }
		let resized = divider.frame.midX - window.frame.minX
		app.typeText("printf '%s' $$ >'\(after.path)'\n")
		waitUntil { FileManager.default.fileExists(atPath: after.path) }
		XCTAssertEqual(try String(contentsOf: before, encoding: .utf8), try String(contentsOf: after, encoding: .utf8), "opening and dragging must preserve the shell process")
		app.terminate()
		app.launch()
		XCTAssertTrue(divider.waitForExistence(timeout: 15))
		waitUntil { abs(divider.frame.midX - window.frame.minX - resized) < 4 }
		divider.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click(forDuration: 0.1,
			thenDragTo: window.coordinate(withNormalizedOffset: CGVector(dx: 0.01, dy: 0.5)))
		waitUntil { abs(divider.frame.minX - window.frame.minX - 240) < 5 }
		divider.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click(forDuration: 0.1,
			thenDragTo: window.coordinate(withNormalizedOffset: CGVector(dx: 0.99, dy: 0.5)))
		waitUntil { abs(window.frame.maxX - divider.frame.maxX - 200) < 5 }
		let restored = scratch.appending(path: "pid-restored"), stacked = scratch.appending(path: "pid-stacked")
		app.typeText("printf '%s' $$ >'\(restored.path)'\n")
		waitUntil { FileManager.default.fileExists(atPath: restored.path) }
		app.typeText("'\(debi)' preview --stack '\(second.path)'\n")
		waitUntil { app.buttons.matching(identifier: "Close preview").count == 2 }
		let capture = XCTAttachment(screenshot: app.screenshot())
		capture.name = "Stacked previews after divider drag"
		capture.lifetime = .keepAlways
		add(capture)
		app.buttons.matching(identifier: "Close preview").element(boundBy: 0).click()
		waitUntil { app.buttons.matching(identifier: "Close preview").count == 1 }
		app.webViews.firstMatch.waitForText("Second pane", timeout: 15)
		app.typeText("printf '%s' $$ >'\(stacked.path)'\n")
		waitUntil { FileManager.default.fileExists(atPath: stacked.path) }
		XCTAssertEqual(try String(contentsOf: restored, encoding: .utf8), try String(contentsOf: stacked, encoding: .utf8))
		app.buttons["Close preview"].firstMatch.click()
		XCTAssertTrue(app.buttons["Close preview"].waitForNonExistence(timeout: 5))
	}

	@MainActor
	private func waitUntil(_ condition: () -> Bool) {
		let deadline = Date.now.addingTimeInterval(10)
		while !condition(), Date.now < deadline { Thread.sleep(forTimeInterval: 0.1) }
		XCTAssertTrue(condition())
	}

	@MainActor
	private func exercisePreview(extension suffix: String, contents: String, format: String) throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "preview-ui-\(UUID())")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		defer { try? FileManager.default.removeItem(at: scratch) }
		let config = try writeConfigWithAProfile(in: scratch)
		let file = scratch.appending(path: "preview.\(suffix)")
		try contents.write(to: file, atomically: true, encoding: .utf8)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		defer { app.terminate() }
		app.openFirstShell()
		let debi = try XCTUnwrap(ProcessInfo.processInfo.environment["SAPPHIRE_TEST_DEBI"])
		app.typeText("'\(debi)' preview '\(file.path)'\n")
		let close = app.buttons["Close preview"].firstMatch
		XCTAssertTrue(close.waitForExistence(timeout: 10))
		app.webViews.firstMatch.waitForText("Preview document", timeout: 15, "the \(format) heading is rendered before capture")
		let capture = XCTAttachment(screenshot: app.screenshot())
		capture.name = "\(format) preview beside terminal"
		capture.lifetime = .keepAlways
		add(capture)
		close.click()
		XCTAssertTrue(close.waitForNonExistence(timeout: 5))
		let output = scratch.appending(path: "focused")
		app.typeText("printf focused >'\(output.path)'\n")
		let deadline = Date.now.addingTimeInterval(10)
		while !FileManager.default.fileExists(atPath: output.path), Date.now < deadline { Thread.sleep(forTimeInterval: 0.1) }
		XCTAssertEqual(try String(contentsOf: output, encoding: .utf8), "focused")
	}
}
