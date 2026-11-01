import XCTest

/// The New Task dialog with real input: typing in its fields never reaches the hub's keys, Enter toggles the active repo in the filter and creates the task in the title, and Tab stays within the dialog.
final class NewTaskDialogUITests: XCTestCase {
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

	/// A profile with the repos `api` and `web`, and no workspaces.
	@MainActor
	private func launchWithProfile() throws {
		// The runner's sandbox can't run git, so `scripts/test-ui.sh` makes the repo.
		let template = try XCTUnwrap(
			ProcessInfo.processInfo.environment["SAPPHIRE_GIT_REPO"], "run the XCUITests with scripts/test-ui.sh, which makes their git repo")
		let profile = scratch.appending(path: "Work")
		for repo in ["api", "web"] {
			try FileManager.default.createDirectory(at: profile.appending(path: "repos"), withIntermediateDirectories: true)
			try FileManager.default.copyItem(at: URL(filePath: template), to: profile.appending(path: "repos/\(repo)"))
		}
		try FileManager.default.createDirectory(at: profile.appending(path: "workspaces"), withIntermediateDirectories: true)
		try #"{"name": "Work"}"#.write(to: profile.appending(path: "config.json"), atomically: true, encoding: .utf8)
		let config = scratch.appending(path: "config.json")
		try #"{"profiles": ["\#(profile.path)"]}"#.write(to: config, atomically: true, encoding: .utf8)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
	}

	@MainActor
	func testTypingStaysInTheDialogEnterTogglesInTheFilterAndCreatesFromTheTitle() throws {
		try launchWithProfile()
		XCTAssertTrue(app.staticTexts["No workspaces found"].waitForExistence(timeout: 10))

		app.typeText("n")
		let dialog = element("new-task-dialog")
		XCTAssertTrue(dialog.waitForExistence(timeout: 5))
		let title = element("new-task-title")
		// The hub's keys: `n` would open another dialog, `q` close, `?` the cheatsheet, `f` focus its filter.
		app.typeText("nq?f")
		XCTAssertEqual(title.value as? String, "nq?f", "the title field has the keyboard as the dialog opens")
		XCTAssertFalse(element("hub-cheatsheet").exists)

		// "+ Clone Repo", then the filter.
		app.typeKey(.tab, modifierFlags: [])
		app.typeKey(.tab, modifierFlags: [])
		app.typeText("we")
		XCTAssertEqual(element("new-task-filter").value as? String, "we")
		let web = element("new-task-repo-web")
		XCTAssertFalse(element("new-task-repo-api").exists, "the filter hides the repos it doesn't match")
		app.typeKey(.return, modifierFlags: [])
		XCTAssertEqual(web.value as? String, "selected", "Enter in the filter toggles the active repo")
		app.typeKey(.return, modifierFlags: [])
		XCTAssertEqual(web.value as? String ?? "", "")
		XCTAssertTrue(dialog.exists)

		// Filter, Cancel, Create, then back to the title, whose text a field selects when tabbed into: the focus never leaves the dialog.
		for _ in 0..<3 { app.typeKey(.tab, modifierFlags: []) }
		app.typeText("Ship it")
		XCTAssertEqual(title.value as? String, "Ship it")
		XCTAssertEqual(element("hub-filter").value as? String ?? "", "", "no key reached the hub beneath")

		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(dialog.waitForNonExistence(timeout: 5))
		XCTAssertTrue(app.hubHeader.waitForNonExistence(timeout: 5), "creating the task closes the hub")
		XCTAssertTrue(app.staticTexts["Ship it"].waitForExistence(timeout: 10), "the task's tab opens, named after it")
		XCTAssertTrue(
			FileManager.default.fileExists(atPath: scratch.appending(path: "Work/workspaces/ws-1/task.json").path),
			"the task's workspace is created")
	}

	@MainActor
	func testEscapeClosesTheDialogButNotTheHub() throws {
		try launchWithProfile()
		XCTAssertTrue(app.staticTexts["No workspaces found"].waitForExistence(timeout: 10))

		app.buttons["+ New Task"].click()
		let dialog = element("new-task-dialog")
		XCTAssertTrue(dialog.waitForExistence(timeout: 5))
		app.typeKey(.escape, modifierFlags: [])

		XCTAssertTrue(dialog.waitForNonExistence(timeout: 5))
		XCTAssertTrue(app.hubHeader.exists)
	}

	/// The Clone Repo dialog over the New Task dialog: its URL field has the keyboard, Escape closes it alone, and Enter clones, after which the New Task list has the clone, selected.
	@MainActor
	func testCloneRepoTakesTheKeysAndItsCloneJoinsTheNewTaskList() throws {
		try launchWithProfile()
		XCTAssertTrue(app.staticTexts["No workspaces found"].waitForExistence(timeout: 10))
		app.typeText("n")
		let newTask = element("new-task-dialog")
		XCTAssertTrue(newTask.waitForExistence(timeout: 5))

		// The title, then "+ Clone Repo", which Space presses.
		app.typeKey(.tab, modifierFlags: [])
		app.typeKey(.space, modifierFlags: [])
		let clone = element("clone-repo-dialog")
		XCTAssertTrue(clone.waitForExistence(timeout: 5))
		let url = element("clone-repo-url")
		app.typeText("q")
		XCTAssertEqual(url.value as? String, "q", "the URL field has the keyboard as the dialog opens, and q types")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(clone.waitForNonExistence(timeout: 5))
		XCTAssertTrue(newTask.exists, "Escape closes the Clone Repo dialog alone")

		element("new-task-clone-repo").click()
		XCTAssertTrue(clone.waitForExistence(timeout: 5))
		let template = try XCTUnwrap(ProcessInfo.processInfo.environment["SAPPHIRE_GIT_REPO"])
		app.typeText("file://\(template)")
		let hint = element("clone-repo-hint")
		XCTAssertTrue(hint.waitForExistence(timeout: 5))
		XCTAssertEqual(hint.value as? String, "Clones to:  \(scratch.appending(path: "Work/repos/repo").path)")

		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(clone.waitForNonExistence(timeout: 30), "the clone closes its dialog once done")
		let cloned = element("new-task-repo-repo")
		XCTAssertTrue(cloned.waitForExistence(timeout: 5))
		XCTAssertEqual(cloned.value as? String, "selected")
		XCTAssertTrue(FileManager.default.fileExists(atPath: scratch.appending(path: "Work/repos/repo/.git").path))
	}

	@MainActor
	private func element(_ identifier: String) -> XCUIElement {
		app.descendants(matching: .any)[identifier].firstMatch
	}
}
