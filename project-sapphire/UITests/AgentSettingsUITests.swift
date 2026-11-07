import XCTest

final class AgentSettingsUITests: XCTestCase {
	@MainActor
	func testMultilineSaveAndTemplateFilePicker() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-agent-settings-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		let config = try writeConfigWithAProfile(in: scratch)
		let profileConfig = scratch.appending(path: "Work/config.json")
		let template = scratch.appending(path: "agent-template.md")
		try "You are {{agent.name}}.".write(to: template, atomically: true, encoding: .utf8)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_TEST_MODE"] = "1"
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor in
			app.terminate()
			try? FileManager.default.removeItem(at: scratch)
		}
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10), "The initial hub must load")
		app.typeKey("p", modifierFlags: .shift)
		let roleMode = app.descendants(matching: .any)["agent-mode-role"].firstMatch
		XCTAssertTrue(roleMode.waitForExistence(timeout: 10), "The profile Agent card must load")
		let preview = app.descendants(matching: .any)["agent-settings-preview-text"].firstMatch
		XCTAssertTrue(preview.waitForExistence(timeout: 10), "The profile card draws its saved prompt preview")
		XCTAssertTrue((preview.value as? String ?? preview.label).contains("/Work/agent-spaces/<name>"), "The preview uses this profile's space path")
		app.scrollSettingsDetail(to: roleMode)
		roleMode.radioButtons["Set"].click()
		let role = app.textViews["agent-input-role"]
		XCTAssertTrue(role.waitForExistence(timeout: 5), "Set reveals the multiline editor")
		role.click()
		app.typeKey("a", modifierFlags: .command)
		app.typeText("jdk")
		app.typeKey(.return, modifierFlags: [])
		app.typeKey(.return, modifierFlags: .shift)
		app.typeText("next")
		let expected = "jdk\n\nnext"
		XCTAssertEqual(role.value as? String, expected, "Both Return keys insert newlines")
		XCTAssertNil(try agentConfig(profileConfig)["role"], "Typing and Return keep the draft unsaved")
		XCTAssertFalse(app.descendants(matching: .any)["confirmation-dialog"].firstMatch.exists, "d typed into the editor stays text")
		app.buttons["agent-save-role"].click()
		let savedRole = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
			(try? self.agentConfig(profileConfig)["role"] as? String) == expected
		}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [savedRole], timeout: 10), .completed, "Save writes multiline text verbatim")

		let templateMode = app.descendants(matching: .any)["agent-mode-template-file"].firstMatch
		app.scrollSettingsDetail(to: templateMode)
		templateMode.radioButtons["Set"].click()
		let pathInput = app.textFields["agent-input-template-file"]
		XCTAssertTrue(pathInput.waitForExistence(timeout: 5))
		app.buttons["agent-template-browse"].click()
		let picker = app.sheets.firstMatch
		XCTAssertTrue(picker.waitForExistence(timeout: 5), "Browse opens the file picker")
		picker.buttons["Cancel"].click()
		XCTAssertTrue(picker.waitForNonExistence(timeout: 5))
		XCTAssertEqual(pathInput.value as? String, "", "Cancel preserves the path draft")
		app.buttons["agent-template-browse"].click()
		XCTAssertTrue(picker.waitForExistence(timeout: 5))
		app.typeKey("g", modifierFlags: [.command, .shift])
		app.typeText(template.path)
		app.typeKey(.return, modifierFlags: [])
		picker.buttons["Choose Template"].click()
		let selected = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
			app.textFields["agent-input-template-file"].value as? String == template.path
		}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [selected], timeout: 5), .completed, "The chosen path becomes an unsaved draft")
		XCTAssertNil(try agentConfig(profileConfig)["template-file"])
		app.buttons["agent-save-template-file"].click()
		let savedPath = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
			(try? self.agentConfig(profileConfig)["template-file"] as? String) == template.path
		}, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [savedPath], timeout: 10), .completed)
	}

	private func agentConfig(_ file: URL) throws -> [String: Any] {
		let root = try JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any]
		return root?["agent"] as? [String: Any] ?? [:]
	}
}
