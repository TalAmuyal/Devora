import XCTest

/// The native transcript (ADR-014) under real input: following the bottom, staying put while scrolled up, and
/// loading older history at the top without moving what the reader looks at, under the wheel; a card answered by a click.
final class NativeTranscriptUITests: XCTestCase {
	@MainActor
	func testFollowingTheBottomAndLoadingOlderHistoryWithTheWheel() throws {
		let (app, editor, transcript) = try openAgent()

		// The fake agent answers `history:240` with 240 replies, "History turn 0" to "History turn 239": past the chat's
		// 200-row tail, so older history exists, in one turn rather than hundreds of typed prompts.
		send("history:240", from: editor, in: app)
		// The rows reach accessibility a frame or more after they are drawn, so the snapshot is retaken until it catches up.
		var view = try TranscriptSnapshot(transcript)
		_ = poll(timeout: 5) {
			guard let next = try? TranscriptSnapshot(transcript) else { return false }
			view = next
			return next.shows("History turn 239")
		}
		if !view.shows("History turn 239") {
			let dump = XCTAttachment(string: transcript.debugDescription)
			dump.name = "Transcript accessibility tree"
			dump.lifetime = .keepAlways
			add(dump)
		}
		XCTAssertTrue(view.shows("History turn 239"), "Following the bottom shows the newest reply, not \(view.inViewSummary)")
		capture(app, "Native transcript following the bottom")

		// Over a code block, which scrolls only sideways, the wheel still scrolls the transcript up and down, and the block
		// sideways.
		send("wide-code", from: editor, in: app)
		// A row is one accessibility element (its kind, then its text), so the code block is found by its row's text.
		let code = transcript.staticTexts.matching(NSPredicate(format: "value BEGINSWITH %@", "Wide code:")).firstMatch
		XCTAssertTrue(code.waitForExistence(timeout: 10), "The wide code block must be in the transcript")
		// The new rows are still being measured after the turn ends; their corrections move the block, so each position is
		// read once it holds still.
		let codeTop = settledTop(of: code)
		code.scroll(byDeltaX: 0, deltaY: 300)
		let scrolledTop = settledTop(of: code)
		XCTAssertGreaterThan(scrolledTop, codeTop + 100, "A vertical scroll over a code block must scroll the transcript")
		code.scroll(byDeltaX: -300, deltaY: 0)
		XCTAssertEqual(settledTop(of: code), scrolledTop, accuracy: 1, "A sideways scroll over a code block must not scroll the transcript")

		// Scrolled up, a new turn arrives below without moving what the reader looks at.
		transcript.scroll(byDeltaX: 0, deltaY: 600)
		view = try settledSnapshot(of: transcript)
		let reading = try XCTUnwrap(view.topTurn, "A reply must be in view after scrolling up")
		send("History turn 240", from: editor, in: app)
		let after = try TranscriptSnapshot(transcript)
		XCTAssertFalse(after.shows("History turn 240"), "Scrolled up, a new turn must not pull the view down")
		XCTAssertEqual(
			after.top(of: reading.text) ?? .nan, reading.top, accuracy: 1,
			"The row in view must stay put (\(reading.text) in the snapshot at \(after.turns.filter { $0.text == reading.text }.map(\.top)))")

		// At the top, the older page loads above, and the reader stays on the rows they were reading.
		let tail = after.rows
		var beforeLoad = after
		view = after
		var scrolls = 0
		var steps: [String] = []
		while view.rows == tail && scrolls < 150 {
			beforeLoad = view
			// Big steps while one can't reach the top 80 pt, where the older page loads; then half the visible area, so
			// that the row on top before the step that loads stays in view after it, unless the view jumps.
			let step = view.distanceFromTop > 400 + 120 ? 400 : view.visibleArea.height / 2
			transcript.scroll(byDeltaX: 0, deltaY: step)
			view = try settledSnapshot(of: transcript)
			steps.append("\(Int(step)) from \(Int(beforeLoad.distanceFromTop)) to \(Int(view.distanceFromTop))")
			scrolls += 1
		}
		capture(app, "Native transcript after loading older history")
		// After hundreds of rows scrolled through and an older page: `NSTableView` exposes every row, in view or not, so that
		// VoiceOver can reach it; what a reused cell must not leave behind is a row's text twice, or at another row's place.
		XCTAssertEqual(Set(view.turns.map(\.text)).count, view.turns.count, "No row text twice: \(view.turns.map(\.number).sorted())")
		let byNumber = view.turns.sorted { $0.number < $1.number }
		let misplaced = zip(byNumber, byNumber.dropFirst()).filter { $0.top >= $1.top }
		XCTAssertTrue(misplaced.isEmpty, "Every row's element at its row's place: \(misplaced.map { "\($0.number)@\(Int($0.top)) above \($1.number)@\(Int($1.top))" })")
		XCTAssertGreaterThan(view.rows, tail, "Scrolling to the top must load older history")
		let anchor = try XCTUnwrap(beforeLoad.topTurn?.text)
		XCTAssertTrue(
			view.shows(anchor),
			"\(anchor) must still be in view once older history loads (in view before: \(beforeLoad.inViewSummary); after: \(view.inViewSummary); last steps, by distance from the top: \(steps.suffix(3).joined(separator: ", ")))")
		XCTAssertFalse(view.shows("History turn 0"), "Loading older history must not jump to its top")
	}

	/// A permission card's buttons answer its question, as its keys do: the card goes, and the turn goes on.
	@MainActor
	func testAPermissionCardIsAnsweredWithItsButton() throws {
		let (app, editor, transcript) = try openAgent()
		// The fake agent answers `permission` by asking to run Bash, with a suggested rule, and waits for the answer.
		submit("permission", from: editor, in: app)
		let card = transcript.groups["Permission request"]
		XCTAssertTrue(card.waitForExistence(timeout: 15), "The question must show as a card")
		let rule = "Allow Bash(custom-tool), saved to localSettings"
		XCTAssertTrue(
			card.staticTexts.matching(NSPredicate(format: "value == %@ OR label == %@", rule, rule)).firstMatch.exists,
			"The suggested rule reads as a sentence")
		XCTAssertTrue(card.buttons["Always allow ⌘⇧Y"].exists, "A question that can always be allowed offers it")
		capture(app, "Native transcript permission card")
		card.buttons["Allow ⌘Y"].click()
		XCTAssertTrue(poll { !card.exists }, "Answering must remove the card")
		XCTAssertTrue(poll { !app.buttons["■ Stop"].exists }, "The turn must go on to its end once answered")
	}

	/// Launches the app on the native transcript, creates the "Echo" agent (the fake CLI) and focuses its editor.
	@MainActor
	private func openAgent() throws -> (app: XCUIApplication, editor: XCUIElement, transcript: XCUIElement) {
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
		return (app, editor, transcript)
	}

	/// The transcript once a scroll has landed: the same position and rows in two snapshots 100 ms apart (or after 5 s). A
	/// snapshot taken at once can come before the scroll is applied, and a step then seems to move twice as far.
	@MainActor
	private func settledSnapshot(of transcript: XCUIElement) throws -> TranscriptSnapshot {
		var last = try TranscriptSnapshot(transcript)
		var result: TranscriptSnapshot? = nil
		_ = poll(timeout: 5) {
			RunLoop.current.run(until: Date().addingTimeInterval(0.1))
			guard let next = try? TranscriptSnapshot(transcript) else { return false }
			defer { last = next }
			guard next.distanceFromTop == last.distanceFromTop, next.rows == last.rows else { return false }
			result = next
			return true
		}
		return try result ?? TranscriptSnapshot(transcript)
	}

	/// `element`'s top once it has held still for 250 ms (or after 5 s).
	@MainActor
	private func settledTop(of element: XCUIElement) -> CGFloat {
		var top = element.frame.minY
		var still = Date()
		_ = poll(timeout: 5) {
			let now = element.frame.minY
			if now != top { top = now; still = Date() }
			return Date().timeIntervalSince(still) >= 0.25
		}
		return top
	}

	/// Sends `prompt` and waits for its turn to end.
	@MainActor
	private func send(_ prompt: String, from editor: XCUIElement, in app: XCUIApplication) {
		submit(prompt, from: editor, in: app)
		XCTAssertTrue(poll { (editor.value as? String) == "" && !app.buttons["■ Stop"].exists }, "\(prompt) must complete")
	}

	/// Sends `prompt`, without waiting for its turn.
	@MainActor
	private func submit(_ prompt: String, from editor: XCUIElement, in app: XCUIApplication) {
		editor.typeText(prompt)
		XCTAssertTrue(poll { app.buttons["Send"].isEnabled }, "The agent must be ready for \(prompt)")
		editor.typeKey(.return, modifierFlags: .command)
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
///
/// A text can be in the snapshot more than once (a row's stale copy outside the visible area), so every occurrence is
/// kept, and the one in view is the one that counts.
private struct TranscriptSnapshot {
	struct Turn {
		let text: String
		let frame: CGRect
		var top: CGFloat { frame.minY }
		var number: Int { Int(text.dropFirst("History turn ".count)) ?? 0 }
	}

	/// The transcript's visible area, and the turns it has (in view or not), with their frames.
	let visibleArea: CGRect
	let turns: [Turn]
	let rows: Int
	/// How far the visible area's top is below the first row's, the transcript's top (every row is in the snapshot).
	let distanceFromTop: CGFloat

	@MainActor
	init(_ scrollView: XCUIElement) throws {
		let snapshot = try scrollView.snapshot()
		visibleArea = snapshot.frame
		var turns: [Turn] = []
		var rows = 0
		var contentTop = CGFloat.infinity
		func walk(_ element: XCUIElementSnapshot) {
			if element.elementType == .tableRow {
				rows += 1
				contentTop = min(contentTop, element.frame.minY)
			}
			// A row is one element: its kind as the label, its text as the value.
			let text = element.value as? String ?? element.label
			if text.hasPrefix("History turn ") {
				turns.append(Turn(text: text, frame: element.frame))
			}
			element.children.forEach(walk)
		}
		walk(snapshot)
		self.turns = turns
		self.rows = rows
		distanceFromTop = contentTop.isFinite ? visibleArea.minY - contentTop : 0
	}

	private func inView(_ turn: Turn) -> Bool {
		visibleArea.contains(CGPoint(x: turn.frame.midX, y: turn.frame.midY))
	}

	private var turnsInView: [Turn] { turns.filter(inView) }

	func shows(_ text: String) -> Bool { turnsInView.contains { $0.text == text } }

	/// The top of `text` in view, or nil when it isn't.
	func top(of text: String) -> CGFloat? { turnsInView.first { $0.text == text }?.top }

	/// The turns in view, top to bottom, as `<number>@<top>`: what a failure shows.
	var inViewSummary: String { turnsInView.sorted { $0.top < $1.top }.map { "\($0.number)@\(Int($0.top))" }.joined(separator: ", ") }

	/// The topmost turn in view.
	var topTurn: Turn? { turnsInView.min { $0.top < $1.top } }
}
