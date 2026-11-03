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
		let heading = app.webViews.firstMatch.staticTexts.matching(NSPredicate(format: "value CONTAINS %@", "Preview document")).firstMatch
		XCTAssertTrue(heading.waitForExistence(timeout: 15), "the \(format) heading is rendered before capture")
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
