import XCTest

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
		buttons["New Shell"].click()
		XCTAssertTrue(staticTexts.matching(identifier: "Shell").firstMatch.waitForExistence(timeout: 10))
	}
}
