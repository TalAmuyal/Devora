import XCTest

/// The Settings Hub with real input: typing into a settings card's input, where Enter commits and the list's keys (`j`, `d`) type rather than act, none of which the test bridge can drive.
final class SettingsHubUITests: XCTestCase {
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

	@MainActor
	func testTypingIntoACardInputCommitsOnEnterAndNeverReachesTheList() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		try app.useTheFakeClaude(reportingIn: scratch)
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))

		app.typeKey("p", modifierFlags: .shift)
		let settings = element("settings-hub")
		XCTAssertTrue(settings.waitForExistence(timeout: 5), "P opens the Settings Hub")
		let segments = element("config-segments-terminal.default-app")
		XCTAssertTrue(segments.waitForExistence(timeout: 5), "the active profile's cards load")
		app.scrollSettingsDetail(to: segments)
		segments.radioButtons["Set"].click()

		let input = element("config-input-terminal.default-app")
		XCTAssertTrue(input.waitForExistence(timeout: 5), "Set reveals the input")
		XCTAssertTrue(waitFor(input, "hasKeyboardFocus == true"), "and gives it the focus")
		app.typeText("jdk")
		app.typeKey(.return, modifierFlags: [])

		let profileConfig = scratch.appending(path: "Work/config.json")
		XCTAssertTrue(
			waitFor(input, "value == 'jdk'"), "j and d typed into the input stay in it")
		XCTAssertFalse(element("confirmation-dialog").exists, "d typed into the input doesn't delete the profile")
		let stored = XCTNSPredicateExpectation(
			predicate: NSPredicate { _, _ in
				(try? String(contentsOf: profileConfig, encoding: .utf8))?.contains(#""default-app": "jdk""#) == true
			}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [stored], timeout: 5), .completed, "Enter commits the value to the profile's config")
		XCTAssertTrue(input.exists, "the list's cursor stayed on the profile")

		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(waitFor(input, "hasKeyboardFocus == false"), "Escape leaves the input")
		XCTAssertTrue(settings.exists, "and doesn't close the Settings Hub")
		app.typeKey("j", modifierFlags: [])
		XCTAssertTrue(element("profile-form-name").waitForExistence(timeout: 5), "out of the input, j moves to the New Profile row")
	}

	@MainActor
	func testTypingACustomClaudeModelCommitsToTheSelectedProfile() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		try app.useTheFakeClaude(reportingIn: scratch)
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		app.typeKey("p", modifierFlags: .shift)
		let segments = element("claude-segments-opus-model")
		XCTAssertTrue(segments.waitForExistence(timeout: 5), app.debugDescription)
		segments.radioButtons["Custom"].click()
		let input = element("claude-input-opus-model")
		XCTAssertTrue(input.waitForExistence(timeout: 5))
		XCTAssertTrue(waitFor(input, "hasKeyboardFocus == true"))
		app.typeKey("a", modifierFlags: .command)
		app.typeText("custom-jdk-model")
		app.typeKey(.return, modifierFlags: [])
		let profileConfig = scratch.appending(path: "Work/config.json")
		let saved = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
			guard let data = try? Data(contentsOf: profileConfig),
			      let config = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
			else { return false }
			return (config["claude"] as? [String: String])?["opus-model"] == "custom-jdk-model"
		}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [saved], timeout: 5), .completed)
		XCTAssertTrue(waitFor(input, "value == 'custom-jdk-model'"))
		XCTAssertFalse(element("confirmation-dialog").exists, "j and d typed into the model id stay in the field")
		let user = try JSONSerialization.jsonObject(with: Data(contentsOf: config)) as? [String: Any]
		XCTAssertNil(user?["claude"], "editing a profile leaves User Defaults alone")
		let screenshot = XCTAttachment(screenshot: app.screenshot())
		screenshot.name = "Claude Launch Settings — custom model"
		screenshot.lifetime = .keepAlways
		add(screenshot)
	}

	/// At the window's default size, its narrowest, the Effort control's seven segments need more than the detail pane
	/// had: the detail is as wide as its widest control from the start, and neither the card nor a page change moves it.
	@MainActor
	func testTheDetailStaysInItsPaneWhenACustomFieldOpens() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		try app.useTheFakeClaude(reportingIn: scratch)
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		app.typeKey("p", modifierFlags: .shift)
		let segments = element("claude-segments-opus-model")
		XCTAssertTrue(segments.waitForExistence(timeout: 5) && element("claude-segments-effort").exists, "the active profile's Claude card loads")
		let detail = element("settings-detail-scroll")
		let window = app.windows.firstMatch.frame
		let opened = detail.frame
		XCTAssertTrue(window.contains(opened), "The detail \(opened) must be inside the window \(window)")
		XCTAssertGreaterThanOrEqual(
			opened.width, element("claude-settings-card").frame.width, "The Claude card must fit its detail, Effort included")

		segments.radioButtons["Custom"].click()
		XCTAssertTrue(element("claude-input-opus-model").waitForExistence(timeout: 5), "Custom reveals the field")
		XCTAssertEqual(detail.frame, opened, "Opening a Custom field must not move or widen the detail")
		app.typeKey(.escape, modifierFlags: [])
		segments.radioButtons["Default"].click()
		XCTAssertTrue(element("claude-input-opus-model").waitForNonExistence(timeout: 5), "Default hides the field")
		XCTAssertEqual(detail.frame, opened, "Closing it must not either")

		// The New Profile page has no Claude card, yet takes the same minimum (`EffortWidth`): the split stays.
		app.typeKey("j", modifierFlags: [])
		XCTAssertTrue(element("profile-form-name").waitForExistence(timeout: 5), "j moves to the New Profile row")
		XCTAssertEqual(detail.frame, opened, "The New Profile page must keep the same split")
		app.typeKey("k", modifierFlags: [])
		XCTAssertTrue(segments.waitForExistence(timeout: 5), "k moves back to the profile")
		XCTAssertEqual(detail.frame, opened, "And so must the profile, again")
	}

	/// Opened straight onto "New Profile…", the one page without a Claude card on screen first, the detail already has
	/// the minimum a profile's page needs: moving to the profile keeps the split.
	@MainActor
	func testOpeningOnNewProfileGivesTheSplitAProfileNeeds() throws {
		let config = try writeConfigWithAProfile(in: scratch)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		let inputTrace = scratch.appending(path: "input-trace.log")
		app.launchEnvironment["DEVORA_INPUT_TRACE"] = inputTrace.path
		app.allowSlowShiftDoubleTaps()
		try app.useTheFakeClaude(reportingIn: scratch)
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10))
		// The palette opens only with no window surface up: the launch's Workspace Hub closes first.
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(app.hubHeader.waitForNonExistence(timeout: 5), "Escape closes the launch's Workspace Hub")
		shiftShift(
			in: app, typing: "New Profile", opens: element("command-palette-search"), inputTrace: inputTrace,
			failure: "Shift-Shift must open the command palette")
		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(element("profile-form-name").waitForExistence(timeout: 10), "The palette's New Profile opens its page")
		let detail = element("settings-detail-scroll")
		let window = app.windows.firstMatch.frame
		let opened = detail.frame
		XCTAssertTrue(window.contains(opened), "The detail \(opened) must be inside the window \(window)")

		app.typeKey("k", modifierFlags: [])
		XCTAssertTrue(element("claude-segments-effort").waitForExistence(timeout: 5), "k moves to the profile, with its Claude card")
		XCTAssertEqual(detail.frame, opened, "The profile's page must keep the split New Profile opened with")
		XCTAssertGreaterThanOrEqual(opened.width, element("claude-settings-card").frame.width, "The Claude card must fit it")
	}

	@MainActor
	private func waitFor(_ element: XCUIElement, _ predicate: String) -> Bool {
		XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: predicate), object: element)], timeout: 5) == .completed
	}

	@MainActor
	private func element(_ identifier: String) -> XCUIElement {
		app.descendants(matching: .any)[identifier].firstMatch
	}
}
