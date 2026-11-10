import XCTest

/// The native transcript (ADR-014) under real wheel input: following the bottom, staying put while scrolled up, and
/// loading older history at the top without moving what the reader looks at.
final class NativeTranscriptUITests: XCTestCase {
	@MainActor
	func testFollowingTheBottomAndLoadingOlderHistoryWithTheWheel() throws {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-transcript-uitest-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		let app = XCUIApplication()
		app.launchArguments = ["-DevoraNativeTranscript", "YES"]
		app.launchEnvironment["DEVORA_TEST_MODE"] = "1"
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = try writeConfigWithAProfile(in: scratch).path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		for key in ["DEVORA_TEST_CLAUDE_PROGRAM", "DEVORA_FAKE_FIXTURES"] {
			app.launchEnvironment[key] = try XCTUnwrap(ProcessInfo.processInfo.environment[key])
		}
		app.launchEnvironment["DEVORA_FAKE_REPORT"] = scratch.appending(path: "fake-report.jsonl").path
		app.launch()
		addTeardownBlock { @MainActor in
			if app.state != .notRunning {
				app.typeKey("q", modifierFlags: .command)
				XCTAssertTrue(app.wait(for: .notRunning, timeout: 10), "Quit must reap the fake agent before fixture removal")
			}
			if app.state == .notRunning { try? FileManager.default.removeItem(at: scratch) }
		}
		XCTAssertTrue(app.hubHeader.waitForExistence(timeout: 10), "The launch hub must appear")
		app.typeKey("s", modifierFlags: .control)
		XCTAssertTrue(app.buttons["+ New Agent"].waitForExistence(timeout: 10), "The Agent Hub must open")
		app.typeKey("n", modifierFlags: [])
		let name = app.textFields["new-agent-name"]
		XCTAssertTrue(name.waitForExistence(timeout: 10), "The New Agent dialog must open")
		name.click()
		name.typeText("Echo")
		app.buttons["Create"].click()
		let editor = app.textViews["agent-prompt"]
		XCTAssertTrue(editor.waitForExistence(timeout: 15), "Creating an agent must open its editor")
		let transcript = app.scrollViews["agent-transcript-scroll"]
		XCTAssertTrue(transcript.waitForExistence(timeout: 15), "The native transcript must replace the web one")
		editor.click()

		// The fake agent answers `history:240` with 240 replies, "History turn 0" to "History turn 239": past the chat's
		// 200-row tail, so older history exists, in one turn rather than hundreds of typed prompts.
		send("history:240", from: editor, in: app)
		var view = try TranscriptSnapshot(transcript)
		XCTAssertTrue(view.shows("History turn 239"), "Following the bottom shows the newest reply")
		capture(app, "Native transcript following the bottom")

		// Over a code block, which scrolls only sideways, the wheel still scrolls the transcript up and down, and the block
		// sideways.
		send("wide-code", from: editor, in: app)
		let code = app.scrollViews["transcript-code-block"].firstMatch
		XCTAssertTrue(code.waitForExistence(timeout: 10), "The wide code block must be in the transcript")
		let codeTop = code.frame.minY
		code.scroll(byDeltaX: 0, deltaY: 300)
		XCTAssertGreaterThan(code.frame.minY, codeTop + 100, "A vertical scroll over a code block must scroll the transcript")
		let movedTop = code.frame.minY
		code.scroll(byDeltaX: -300, deltaY: 0)
		XCTAssertEqual(code.frame.minY, movedTop, accuracy: 1, "A sideways scroll over a code block must not scroll the transcript")

		// Scrolled up, a new turn arrives below without moving what the reader looks at.
		transcript.scroll(byDeltaX: 0, deltaY: 600)
		view = try TranscriptSnapshot(transcript)
		let reading = try XCTUnwrap(view.topTurn, "A reply must be in view after scrolling up")
		send("History turn 240", from: editor, in: app)
		let after = try TranscriptSnapshot(transcript)
		XCTAssertFalse(after.shows("History turn 240"), "Scrolled up, a new turn must not pull the view down")
		XCTAssertEqual(after.top(of: reading.text) ?? .nan, reading.top, accuracy: 1, "The row in view must stay put")

		// At the top, the older page loads above, and the reader stays on the rows they were reading.
		let tail = after.rows
		var beforeLoad = after
		view = after
		var scrolls = 0
		while view.rows == tail && scrolls < 150 {
			beforeLoad = view
			// Big steps until the oldest loaded turn is near, then steps much smaller than the visible area, so that only a
			// jump (not the step itself) could take the row the reader looks at out of view.
			let near = view.oldestTurn.map { oldest in view.topTurn.map { $0.number - oldest <= 3 } ?? false } ?? false
			transcript.scroll(byDeltaX: 0, deltaY: near ? 30 : 400)
			view = try TranscriptSnapshot(transcript)
			scrolls += 1
		}
		capture(app, "Native transcript after loading older history")
		XCTAssertGreaterThan(view.rows, tail, "Scrolling to the top must load older history")
		let anchor = try XCTUnwrap(beforeLoad.topTurn?.text)
		XCTAssertTrue(view.shows(anchor), "\(anchor) must still be in view once older history loads")
		XCTAssertFalse(view.shows("History turn 0"), "Loading older history must not jump to its top")
	}

	@MainActor
	private func send(_ prompt: String, from editor: XCUIElement, in app: XCUIApplication) {
		editor.typeText(prompt)
		XCTAssertTrue(poll { app.buttons["Send"].isEnabled }, "The agent must be ready for \(prompt)")
		editor.typeKey(.return, modifierFlags: .command)
		XCTAssertTrue(poll { (editor.value as? String) == "" && !app.buttons["■ Stop"].exists }, "\(prompt) must complete")
	}

	/// Polls `condition` every 50 ms for up to `timeout` seconds; XCTest's own expectations poll about once a second.
	@MainActor
	private func poll(timeout: TimeInterval = 15, _ condition: () -> Bool) -> Bool {
		let deadline = Date().addingTimeInterval(timeout)
		while !condition() {
			if Date() > deadline { return false }
			RunLoop.current.run(until: Date().addingTimeInterval(0.05))
		}
		return true
	}

	@MainActor
	private func capture(_ app: XCUIApplication, _ name: String) {
		let attachment = XCTAttachment(screenshot: app.windows.firstMatch.screenshot())
		attachment.name = name
		attachment.lifetime = .keepAlways
		add(attachment)
	}
}

/// The transcript as one accessibility snapshot: reading each element's frame separately costs a round trip per element,
/// which over hundreds of rows takes minutes.
private struct TranscriptSnapshot {
	struct Turn {
		let text: String
		let top: CGFloat
		var number: Int { Int(text.dropFirst("History turn ".count)) ?? 0 }
	}

	/// The transcript's visible area, and the prompts it has (in view or not), with their tops.
	let visibleArea: CGRect
	let turns: [Turn]
	let rows: Int

	@MainActor
	init(_ scrollView: XCUIElement) throws {
		let snapshot = try scrollView.snapshot()
		visibleArea = snapshot.frame
		var turns: [Turn] = []
		var rows = 0
		var frames: [String: CGRect] = [:]
		func walk(_ element: XCUIElementSnapshot) {
			if element.elementType == .tableRow { rows += 1 }
			let text = element.label.isEmpty ? (element.value as? String ?? "") : element.label
			if element.elementType == .staticText && text.hasPrefix("History turn ") {
				turns.append(Turn(text: text, top: element.frame.minY))
				frames[text] = element.frame
			}
			element.children.forEach(walk)
		}
		walk(snapshot)
		self.turns = turns
		self.rows = rows
		self.frames = frames
	}

	private let frames: [String: CGRect]

	func shows(_ text: String) -> Bool {
		guard let frame = frames[text] else { return false }
		return visibleArea.contains(CGPoint(x: frame.midX, y: frame.midY))
	}

	func top(of text: String) -> CGFloat? { frames[text]?.minY }

	/// The oldest turn the transcript has loaded.
	var oldestTurn: Int? { turns.map(\.number).min() }

	/// The topmost prompt in view.
	var topTurn: Turn? { turns.filter { shows($0.text) }.min { $0.top < $1.top } }
}
