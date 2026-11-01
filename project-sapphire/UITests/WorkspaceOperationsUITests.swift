import XCTest

/// The Workspace Hub's operations with real input: the Remove Task confirmation's keys (the Tab trap, Escape, Enter), and the header dropdowns' rows, clicked where they are drawn.
final class WorkspaceOperationsUITests: XCTestCase {
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

	/// A profile with an active workspace (`ws-1`) whose repo, `api`, is on a branch with an untracked file, so removing its task asks first.
	@MainActor
	private func makeProfile(_ name: String, title: String) throws -> URL {
		// The runner's sandbox can't run git, so `scripts/test-ui.sh` makes the repo.
		let template = try XCTUnwrap(
			ProcessInfo.processInfo.environment["SAPPHIRE_GIT_REPO"], "run the XCUITests with scripts/test-ui.sh, which makes their git repo")
		let profile = scratch.appending(path: name)
		try FileManager.default.createDirectory(at: profile.appending(path: "repos"), withIntermediateDirectories: true)
		try #"{"name": "\#(name)"}"#.write(to: profile.appending(path: "config.json"), atomically: true, encoding: .utf8)
		let workspace = profile.appending(path: "workspaces/ws-1")
		try FileManager.default.createDirectory(at: workspace, withIntermediateDirectories: true)
		try Data().write(to: workspace.appending(path: "initialized"))
		try #"{"title": "\#(title)"}"#.write(to: workspace.appending(path: "task.json"), atomically: true, encoding: .utf8)
		let repo = workspace.appending(path: "api")
		try FileManager.default.copyItem(at: URL(filePath: template), to: repo)
		try Data().write(to: repo.appending(path: "untracked.txt"))
		return profile
	}

	@MainActor
	private func launch(profiles: [URL]) throws {
		let config = scratch.appending(path: "config.json")
		let paths = profiles.map { "\"\($0.path)\"" }.joined(separator: ", ")
		try #"{"profiles": [\#(paths)]}"#.write(to: config, atomically: true, encoding: .utf8)
		app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = config.path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		app.launch()
		addTeardownBlock { @MainActor [app] in app?.terminate() }
	}

	@MainActor
	func testTheRemoveTaskConfirmationTrapsTabCancelsOnEscapeAndConfirmsOnEnter() throws {
		let profile = try makeProfile("Work", title: "Fix alpha")
		let task = profile.appending(path: "workspaces/ws-1/task.json")
		try launch(profiles: [profile])
		XCTAssertTrue(app.staticTexts["+1 untracked"].waitForExistence(timeout: 10), "the repo's status loads")

		app.buttons["Remove Task"].click()
		let dialog = element("confirmation-dialog")
		XCTAssertTrue(dialog.waitForExistence(timeout: 5))
		let confirm = dialog.buttons["Remove Task"]
		let cancel = dialog.buttons["Cancel"]
		XCTAssertEqual(confirm.value as? String, "focused")

		app.typeKey(.tab, modifierFlags: [])
		XCTAssertEqual(cancel.value as? String, "focused")
		app.typeKey(.tab, modifierFlags: [])
		XCTAssertEqual(confirm.value as? String, "focused", "Tab cycles within the dialog")
		app.typeKey(.tab, modifierFlags: .shift)
		XCTAssertEqual(cancel.value as? String, "focused")
		app.typeText("x")
		XCTAssertEqual(element("hub-filter").value as? String ?? "", "", "no key reaches the hub beneath")

		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(dialog.waitForNonExistence(timeout: 5))
		XCTAssertTrue(app.hubHeader.exists, "Escape closes only the dialog")
		XCTAssertTrue(FileManager.default.fileExists(atPath: task.path))

		app.buttons["Remove Task"].click()
		XCTAssertTrue(dialog.waitForExistence(timeout: 5))
		app.typeKey(.tab, modifierFlags: [])
		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(dialog.waitForNonExistence(timeout: 5))
		XCTAssertTrue(app.staticTexts["No workspaces found"].waitForExistence(timeout: 10), "Enter confirms, whichever button has the focus")
		XCTAssertFalse(FileManager.default.fileExists(atPath: task.path))
	}

	@MainActor
	func testTheDropdownRowsAreClickedWhereTheyAreDrawn() throws {
		let work = try makeProfile("Work", title: "Fix alpha")
		let home = try makeProfile("Home", title: "Fix the porch")
		try launch(profiles: [work, home])
		XCTAssertTrue(app.staticTexts["Fix alpha"].waitForExistence(timeout: 10))

		element("hub-profile-dropdown").click()
		let profiles = element("hub-dropdown-profiles")
		XCTAssertTrue(profiles.waitForExistence(timeout: 5))
		XCTAssertTrue(profiles.buttons["Manage Profiles…"].isEnabled, "Manage Profiles… opens the Settings Hub")
		XCTAssertTrue(profiles.buttons["Work"].isSelected, "the selected profile is checked")
		profiles.buttons["Home"].click()
		XCTAssertTrue(profiles.waitForNonExistence(timeout: 5))
		XCTAssertTrue(app.staticTexts["Fix the porch"].waitForExistence(timeout: 10))

		element("hub-menu").click()
		let menu = element("hub-dropdown-main")
		XCTAssertTrue(menu.waitForExistence(timeout: 5))
		XCTAssertFalse(menu.buttons["Health"].isEnabled, "the Health Hub isn't in Sapphire yet")
		menu.buttons["Save loading latencies"].click()
		XCTAssertTrue(element("hub-toast").waitForExistence(timeout: 5))
		let diagnostics = try FileManager.default.contentsOfDirectory(atPath: home.appending(path: "diagnostics").path)
		XCTAssertEqual(diagnostics.filter { $0.hasPrefix("hub-profile-") }.count, 1)
	}

	@MainActor
	private func element(_ identifier: String) -> XCUIElement {
		app.descendants(matching: .any)[identifier].firstMatch
	}
}
