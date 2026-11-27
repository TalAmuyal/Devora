import XCTest

final class NewAgentDialogUITests: XCTestCase {
	private var scratch: URL!

	override func setUpWithError() throws {
		continueAfterFailure = false
		scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-new-agent-ui-\(UUID().uuidString)")
	}

	override func tearDownWithError() throws {
		try? FileManager.default.removeItem(at: scratch)
	}

	/// A fresh launch opens the window at its smallest, where the form scrolls: a field the focus moves to scrolls fully
	/// into view, with its note, and the repo list with the filter, whose ↑ and ↓ move its cursor.
	@MainActor
	func testTheFocusedFieldScrollsFullyIntoViewInAShortWindow() throws {
		let app = try launchWithRepos((1...12).map { String(format: "repo%02d", $0) })
		app.typeKey("s", modifierFlags: .control)
		XCTAssertTrue(app.buttons["+ New Agent"].waitForExistence(timeout: 10), "The Agent Hub must open")
		app.typeKey("n", modifierFlags: [])
		let form = app.scrollViews["new-agent-form"]
		XCTAssertTrue(form.waitForExistence(timeout: 10), "The New Agent dialog must open")
		let name = app.textFields["new-agent-name"]
		let filter = app.textFields["new-agent-filter"]
		let model = app.textFields["new-agent-model"]
		let note = app.staticTexts["new-agent-model-note"]
		// The list's scroll view: its outline reports the rows' full height, not what shows.
		let list = form.scrollViews.containing(.checkBox, identifier: "new-agent-repo-repo01").firstMatch
		XCTAssertTrue(name.waitForExistence(timeout: 10) && model.exists && note.exists && list.exists)
		XCTAssertFalse(
			form.frame.contains(model.frame), "The window must be short enough that the form scrolls: Model \(model.frame), the form \(form.frame)")

		focus(filter, in: app)
		awaitInView([filter, list], of: form, "the filter, with its list")
		for _ in 1...11 { app.typeKey(.downArrow, modifierFlags: []) }
		let last = app.checkBoxes["new-agent-repo-repo12"]
		awaitInView([last], of: list, "the last repo, at the list's cursor")
		awaitInView([list], of: form, "the list, while its cursor moves")

		app.typeKey(.tab, modifierFlags: [])
		awaitInView([model, note], of: form, "Model, with its note")
		focus(name, in: app, backward: true)
		awaitInView([name], of: form, "Name")
	}

	/// Tabs until `element` has the keyboard focus.
	@MainActor
	private func focus(_ element: XCUIElement, in app: XCUIApplication, backward: Bool = false) {
		for _ in 0..<20 {
			if element.value(forKey: "hasKeyboardFocus") as? Bool == true { return }
			app.typeKey(.tab, modifierFlags: backward ? [.shift] : [])
		}
		XCTFail("Tab never gave \(element.identifier) the focus")
	}

	@MainActor
	private func awaitInView(_ elements: [XCUIElement], of container: XCUIElement, _ what: String) {
		let deadline = Date.now.addingTimeInterval(5)
		repeat {
			if elements.allSatisfy({ container.frame.contains($0.frame) }) { return }
			RunLoop.current.run(until: Date().addingTimeInterval(0.1))
		} while Date.now < deadline
		XCTFail("\(what) must be fully in view: \(elements.map { "\($0.identifier) \($0.frame)" }), in \(container.frame)")
	}

	/// A profile with the given repos, and no workspaces.
	@MainActor
	private func launchWithRepos(_ repos: [String]) throws -> XCUIApplication {
		// The runner's sandbox can't run git, so `scripts/test-ui.sh` makes the repo.
		let template = try XCTUnwrap(
			ProcessInfo.processInfo.environment["SAPPHIRE_GIT_REPO"], "run the XCUITests with scripts/test-ui.sh, which makes their git repo")
		let config = try writeConfigWithAProfile(in: scratch)
		for repo in repos {
			try FileManager.default.copyItem(at: URL(filePath: template), to: scratch.appending(path: "Work/repos/\(repo)"))
		}
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		try app.useTheFakeClaude(reportingIn: scratch)
		app.launch()
		addTeardownBlock { @MainActor in app.terminate() }
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10), "The launch hub must appear")
		return app
	}
}
