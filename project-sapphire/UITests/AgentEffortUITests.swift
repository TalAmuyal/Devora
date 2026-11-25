import XCTest

/// The Effort widget's menu with a real click: it offers the levels the agent's model takes, and a pick reaches the CLI,
/// read from the fake CLI's report.
final class AgentEffortUITests: XCTestCase {
	@MainActor
	func testTheEffortMenuOffersTheModelsLevelsAndAPickReachesTheCLI() throws {
		continueAfterFailure = false
		let (app, editor, scratch) = try launchWithANewAgent(self, scratchName: "sapphire-effort")
		let send = app.buttons["Send"]
		editor.click()
		editor.typeText("first")
		// Send is enabled once the agent runs (and has been offered its models) and the draft isn't empty.
		waitFor("The agent must be ready for its first turn") { send.isEnabled }

		let widget = app.descendants(matching: .any)["agent-effort"]
		waitFor("The widget shows the agent's level once its model's levels are known") {
			(widget.value as? String).map { !$0.contains("Waiting") && !$0.hasPrefix("—") } == true
		}
		widget.click()
		let max = app.menuItems["max"]
		XCTAssertTrue(max.waitForExistence(timeout: 10), "Clicking the widget must open its menu, with the model's levels")
		XCTAssertTrue(app.menuItems["Claude Code default"].exists)
		max.click()

		let report = scratch.appending(path: "fake-report.jsonl")
		waitFor("The pick must reach the CLI, the agent being idle") {
			let told = (try? String(contentsOf: report, encoding: .utf8))?
				.split(separator: "\n")
				.compactMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }
				.filter { $0["kind"] as? String == "apply_flag_settings" }
				.compactMap { ($0["settings"] as? [String: Any])?["effortLevel"] as? String }
			return told == ["max"]
		}
		waitFor("The widget shows the level picked") { (widget.value as? String) == "max" }
	}

	private func waitFor(_ description: String, _ condition: @escaping () -> Bool) {
		let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
		XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 15), .completed, description)
	}
}
