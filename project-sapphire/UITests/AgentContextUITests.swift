import XCTest

/// The Context widget's popover, with real clicks, a real slider drag and Escape. Its screenshots are in the Mac's
/// appearance.
final class AgentContextUITests: XCTestCase {
	@MainActor
	func testContextPopoverSetsTheThresholdAndDisablesItsControlsForAPausedAgent() throws {
		continueAfterFailure = false
		let (app, editor) = try launchWithANewAgent(self, scratchName: "sapphire-context")
		let send = app.buttons["Send"]
		editor.click()
		editor.typeText("first")
		// Send is enabled once the agent runs and the draft isn't empty.
		waitFor("The agent must be ready for its first turn") { send.isEnabled }
		editor.typeKey(.return, modifierFlags: .command)
		let context = app.descendants(matching: .any)["agent-context"]
		let stop = app.buttons["agent-stop"]
		waitFor("The finished turn must report the usage and the window") {
			!stop.exists && (context.value as? String)?.contains(" / ") == true
		}

		context.click()
		let popover = app.popovers.firstMatch
		XCTAssertTrue(popover.waitForExistence(timeout: 10), "Clicking the widget must open its popover")
		let compactNow = app.buttons["agent-compact-now"]
		let slider = app.sliders["agent-compact-threshold"]
		waitFor("Compact Now and the slider are available on an idle agent") { compactNow.isEnabled && slider.isEnabled }
		capture(popover, "the context popover")
		let anchored = popover.frame

		// The middle of 30–100% in steps of 5 is 65%.
		slider.adjust(toNormalizedSliderPosition: 0.5)
		waitFor("Releasing the slider gives this agent its own threshold") {
			(context.value as? String)?.contains("for this agent (profile: 80%)") == true
		}
		// The popover stays open, still anchored where it was: the widget keeps its width as the glyph appears.
		XCTAssertEqual(app.popovers.count, 1, "Setting the threshold must leave the popover open, and only it")
		XCTAssertEqual(popover.frame, anchored, "The popover must not move: the widget it's anchored to keeps its width")
		capture(popover, "the context popover with this agent's own threshold")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(popover.waitForNonExistence(timeout: 10), "Escape must close the popover")
		capture(app.windows.firstMatch, "the widget with the override glyph")

		let pause = app.buttons["agent-pause-control"]
		pause.click()
		waitFor("The agent must pause") { pause.label == "Resume agent" }
		context.click()
		XCTAssertTrue(popover.waitForExistence(timeout: 10), "A paused agent's popover still opens")
		waitFor("A paused agent disables Compact Now and the slider") { !compactNow.isEnabled && !slider.isEnabled }
		capture(popover, "the context popover of a paused agent")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(popover.waitForNonExistence(timeout: 10), "Escape must close the popover")
	}

	@MainActor
	private func capture(_ element: XCUIElement, _ name: String) {
		let screenshot = XCTAttachment(screenshot: element.screenshot())
		screenshot.name = name
		screenshot.lifetime = .keepAlways
		add(screenshot)
	}

	private func waitFor(_ description: String, _ condition: @escaping () -> Bool) {
		let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 15), .completed, description)
	}
}
