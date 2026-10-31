import XCTest

/// Writes a global config in `dir` with one empty profile, and returns its URL: with none, the Workspace Hub a launch opens is the first-run welcome, which can't be closed.
func writeConfigWithAProfile(in dir: URL) throws -> URL {
	let profile = dir.appending(path: "Work")
	for subdirectory in ["repos", "workspaces"] {
		try FileManager.default.createDirectory(at: profile.appending(path: subdirectory), withIntermediateDirectories: true)
	}
	try #"{"name": "Work"}"#.write(to: profile.appending(path: "config.json"), atomically: true, encoding: .utf8)
	let config = dir.appending(path: "config.json")
	try #"{"profiles": ["\#(profile.path)"]}"#.write(to: config, atomically: true, encoding: .utf8)
	return config
}

extension XCUIApplication {
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
