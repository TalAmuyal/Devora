import XCTest

/// The Model widget's popover with real keys: the search field has the focus when it opens, the arrows move the
/// highlight, Return picks it and Escape closes. What the pick sends is read from the fake CLI's report.
final class AgentModelUITests: XCTestCase {
	@MainActor
	func testTheModelPopoverIsDrivenFromTheKeyboardAndAPickReachesTheCLI() throws {
		continueAfterFailure = false
		let (app, editor, scratch) = try launchWithANewAgent(self, scratchName: "sapphire-model")
		let send = app.buttons["Send"]
		editor.click()
		editor.typeText("first")
		// Send is enabled once the agent runs (and has been offered its models) and the draft isn't empty.
		waitFor("The agent must be ready for its first turn") { send.isEnabled }

		let widget = app.descendants(matching: .any)["agent-model"]
		widget.click()
		let popover = app.popovers.firstMatch
		XCTAssertTrue(popover.waitForExistence(timeout: 10), "Clicking the widget must open its popover")
		let search = app.textFields["agent-model-search"]
		waitFor("The search field has the focus when the popover opens") {
			NSPredicate(format: "hasKeyboardFocus == true").evaluate(with: search)
		}
		let anchored = popover.frame
		// Typed without a click: "son" leaves the Sonnet alias, then Sonnet 5.5, 5 and 4.6, the first highlighted.
		app.typeText("son")
		waitFor("Typing narrows the list") {
			!app.descendants(matching: .any)["agent-model-row-opus"].exists
				&& app.descendants(matching: .any)["agent-model-row-claude-sonnet-4-6"].exists
		}
		XCTAssertEqual(popover.frame, anchored, "Narrowing must not move or resize the popover")
		app.typeKey(.downArrow, modifierFlags: [])
		app.typeKey(.downArrow, modifierFlags: [])
		app.typeKey(.upArrow, modifierFlags: [])
		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(popover.waitForNonExistence(timeout: 10), "Return must pick the highlighted model and close the popover")

		let report = scratch.appending(path: "fake-report.jsonl")
		/// The models the fake CLI was told to switch to, in order.
		func told() -> [String]? {
			(try? String(contentsOf: report, encoding: .utf8))?
				.split(separator: "\n")
				.compactMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }
				.filter { $0["kind"] as? String == "set_model" }
				.compactMap { $0["model"] as? String }
		}
		waitFor("The pick must reach the CLI, the agent being idle") { told() == ["claude-sonnet-5-5"] }
		waitFor("The widget names the pick, marked until it answers") {
			(widget.value as? String) == "Sonnet 5.5, no reply from it yet"
		}

		widget.click()
		XCTAssertTrue(popover.waitForExistence(timeout: 10), "The popover opens again")
		app.typeKey(.escape, modifierFlags: [])
		XCTAssertTrue(popover.waitForNonExistence(timeout: 10), "Escape must close the popover")

		// A model's full ID typed exactly shows every row (as a model field does for the value it holds): Return picks the typed model, not the first row.
		widget.click()
		XCTAssertTrue(popover.waitForExistence(timeout: 10), "The popover opens again")
		waitFor("The search field has the focus when the popover opens") {
			NSPredicate(format: "hasKeyboardFocus == true").evaluate(with: search)
		}
		app.typeText("claude-sonnet-4-6")
		app.typeKey(.return, modifierFlags: [])
		XCTAssertTrue(popover.waitForNonExistence(timeout: 10), "Return must pick the typed model and close the popover")
		waitFor("A second pick must reach the CLI") { told()?.count == 2 }
		XCTAssertEqual(told(), ["claude-sonnet-5-5", "claude-sonnet-4-6"], "Return must pick the model whose ID was typed")
	}

	private func waitFor(_ description: String, _ condition: @escaping () -> Bool) {
		let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 15), .completed, description)
	}
}
