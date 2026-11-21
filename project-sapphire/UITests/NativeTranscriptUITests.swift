import XCTest

/// The native transcript (ADR-014) under real input: following the bottom, and the wheel over it and its code blocks,
/// with its rows' accessibility elements where the rows are; a card answered by a click.
/// What the wheel only takes the view to (staying put while scrolled up, loading older history) is tested in process
/// (`TranscriptScrollViewTests`) and, through the whole app, by agent-chat.feature.
final class NativeTranscriptUITests: XCTestCase {
	/// Where the app writes its wheel and scrolling decisions (`DEVORA_WHEEL_TRACE`), attached when a wheel assertion fails.
	private var wheelTrace: URL?

	@MainActor
	func testTheWheelOverTheTranscriptAndItsCodeBlocks() throws {
		let (app, editor, transcript) = try openAgent()

		// The fake agent answers `history:240` with 240 replies, "History turn 0" to "History turn 239": past the chat's
		// 200-row tail, in one turn rather than hundreds of typed prompts.
		send("history:240", from: editor, in: app)
		// The rows reach accessibility a frame or more after they are drawn, so the snapshot is retaken until it catches up.
		var view = try TranscriptSnapshot(transcript)
		_ = poll(timeout: 5) {
			guard let next = try? TranscriptSnapshot(transcript) else { return false }
			view = next
			return next.shows("History turn 239")
		}
		if !view.shows("History turn 239") { attachTree(of: transcript) }
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
		// At an explicit point: scrolling the element itself, XCUITest picks its own point of the row, which is the row's
		// label at its top-left (the block's centre hit-tests to the block's own scroll view), not the code block.
		// Up by 150 over the block's centre, then back down over the label: CI's small window keeps both points in view.
		// XCUITest's delta isn't points (150 moves the view about 85), so the check is that it moved, not how far.
		let centre = code.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
		let overBlock = "at \(centre.screenPoint), code frame \(code.frame), visible area \(transcript.frame)"
		let (scrolledTop, overBlockFrames) = try wheelScroll(
			"the wheel scroll over the code block", aimed: overBlock, in: app, transcript,
			send: { centre.scrollWithPointer(byDeltaX: 0, deltaY: 150) },
			landing: { settledTop(of: code, movedFrom: codeTop) }, landed: { $0 > codeTop + 30 })
		XCTAssertGreaterThan(scrolledTop, codeTop + 30, "A vertical scroll over a code block must scroll the transcript (\(overBlock))\(overBlockFrames)")

		// The row's label is part of the transcript too. The row is one accessibility element, so the label has none of its
		// own to aim at: scrolling the element scrolls at XCUITest's own point for it, which is the label (the case that
		// failed in CI). The app's trace records the point and the view it hit.
		let overLabel = "XCUITest's own point for the row, code frame \(code.frame), visible area \(transcript.frame)"
		let (backTop, overLabelFrames) = try wheelScroll(
			"the wheel scroll over the row's label", aimed: overLabel, in: app, transcript,
			send: { code.scrollWithPointer(byDeltaX: 0, deltaY: -150) },
			landing: { settledTop(of: code, movedFrom: scrolledTop) }, landed: { $0 < scrolledTop - 30 })
		XCTAssertLessThan(backTop, scrolledTop - 30, "A vertical scroll over a row's label must scroll the transcript (\(overLabel))\(overLabelFrames)")

		try awaitDrawing(before: "the sideways scroll over the code block")
		code.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).scrollWithPointer(byDeltaX: -300, deltaY: 0)
		XCTAssertEqual(settledTop(of: code), backTop, accuracy: 1, "A sideways scroll over a code block must not scroll the transcript")

		// Up through about half the 200-row tail (3,000 pt of its 5,000), so that the table reuses its cells for the rows coming
		// into view, and short of the top, where an older page would load.
		let bottom = try TranscriptSnapshot(transcript)
		let (scrolledUp, upFrames) = try wheelScroll(
			"the wheel scrolls up the tail", aimed: "the transcript's centre, 5 × 600", in: app, transcript,
			send: { for _ in 0..<5 { transcript.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).scrollWithPointer(byDeltaX: 0, deltaY: 600) } },
			landing: { try settledSnapshot(of: transcript, after: bottom) },
			landed: { $0.distanceFromTop < bottom.distanceFromTop - 2_000 })
		view = scrolledUp
		capture(app, "Native transcript scrolled up")
		XCTAssertLessThan(view.distanceFromTop, bottom.distanceFromTop - 2_000, "The wheel must scroll the transcript up (from \(Int(bottom.distanceFromTop)) to \(Int(view.distanceFromTop)) below its top)\(upFrames)")
		// `NSTableView` exposes every row, in view or not, so that VoiceOver can reach it; what a reused cell must not leave
		// behind is a row's text twice, or at another row's place.
		XCTAssertEqual(Set(view.turns.map(\.text)).count, view.turns.count, "No row text twice: \(view.turns.map(\.number).sorted())")
		let byNumber = view.turns.sorted { $0.number < $1.number }
		let misplaced = zip(byNumber, byNumber.dropFirst()).filter { $0.top >= $1.top }
		XCTAssertTrue(misplaced.isEmpty, "Every row's element at its row's place: \(misplaced.map { "\($0.number)@\(Int($0.top)) above \($1.number)@\(Int($1.top))" })")
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

	/// Launches the app, creates the "Echo" agent (the fake CLI) and focuses its editor.
	@MainActor
	private func openAgent() throws -> (app: XCUIApplication, editor: XCUIElement, transcript: XCUIElement) {
		continueAfterFailure = false
		let scratch = FileManager.default.temporaryDirectory.appending(path: "sapphire-transcript-uitest-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
		let app = XCUIApplication()
		app.launchEnvironment["DEVORA_CONFIG_PATH"] = try writeConfigWithAProfile(in: scratch).path
		app.launchEnvironment["DEVORA_STATE_DIR"] = scratch.appending(path: "state").path
		try app.useTheFakeClaude(reportingIn: scratch)
		wheelTrace = scratch.appending(path: "wheel-trace.log")
		app.launchEnvironment["DEVORA_WHEEL_TRACE"] = wheelTrace?.path
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
		XCTAssertTrue(transcript.waitForExistence(timeout: 15), "The agent tab must show its transcript")
		editor.click()
		return (app, editor, transcript)
	}

	/// The transcript once a scroll from `before` has landed: first moved (its position or its rows differ from `before`,
	/// unless `before` was already at the top, where a scroll up can't move it), then the same position and rows in two
	/// snapshots 100 ms apart (or after 5 s each). Holding still alone isn't enough: on a slow machine the view also holds
	/// still before a scroll starts, and the next step would then carry two scrolls.
	@MainActor
	private func settledSnapshot(of transcript: XCUIElement, after before: TranscriptSnapshot) throws -> TranscriptSnapshot {
		if before.distanceFromTop > 0 {
			_ = poll(timeout: 5) {
				guard let now = try? TranscriptSnapshot(transcript) else { return false }
				return now.distanceFromTop != before.distanceFromTop || now.rows != before.rows
			}
		}
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

	/// `element`'s top once it has held still for 250 ms (or after 5 s); with `movedFrom`, only once it has left that top
	/// first, since it also holds still before a scroll starts.
	@MainActor
	private func settledTop(of element: XCUIElement, movedFrom start: CGFloat? = nil) -> CGFloat {
		if let start { _ = poll(timeout: 5) { element.frame.minY != start } }
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

	/// The runner, not the transcript, failed the test: the app's window got no display frames, which a wheel's scroll is
	/// animated over (`Packages/TranscriptKit/CLAUDE.md`). An infrastructure failure, which the PR's owner may re-run.
	struct RunnerProblem: Error, CustomStringConvertible {
		let description: String
	}

	/// Sends a wheel scroll once the window draws (`awaitDrawing`), and reads where it landed. A scroll that didn't land
	/// while one of its wheel events got 0 frames (the trace's `frames after the wheel`) was the runner's: it's sent once
	/// more, recorded as a retry, after the window draws again. Returns the landing and, for a scroll that didn't land, what
	/// the assertion's message adds: every wheel event's frame count, per attempt, and whether that was the runner.
	@MainActor
	private func wheelScroll<Landing>(
		_ what: String, aimed aim: String, in app: XCUIApplication, _ transcript: XCUIElement,
		send: () -> Void, landing: () throws -> Landing, landed: (Landing) -> Bool
	) throws -> (Landing, String) {
		var attempts: [[String]] = []
		while true {
			try awaitDrawing(before: what)
			let start = ProcessInfo.processInfo.systemUptime
			send()
			let result = try landing()
			if landed(result) { return (result, "") }
			let counts = wheelFrames(since: start)
			attempts.append(counts)
			let starved = counts.contains { $0.hasPrefix("0 in ") }
			if starved && attempts.count == 1 {
				// The one retry, and only for this measured case: a scroll that failed with frames arriving is the transcript's.
				let note = "Retried \(what): a wheel event of it got no display frames (\(counts.joined(separator: "; ")))"
				print(note)
				let attachment = XCTAttachment(string: note)
				attachment.name = "Wheel retry: \(what)"
				attachment.lifetime = .keepAlways
				add(attachment)
				continue
			}
			attachWheelEvidence(app, transcript, what, aim)
			let perAttempt = attempts.enumerated().map { "attempt \($0 + 1): \($1.joined(separator: "; "))" }.joined(separator: " / ")
			return (result, starved
				? ". Runner problem, not the transcript: a wheel event got no display frames to animate the scroll over (\(perAttempt))"
				: "; frames after each wheel event: \(perAttempt)")
		}
	}

	/// Waits, up to 10 s, until the app's window has drawn a frame since the wait began (the trace's `drawing:` heartbeat),
	/// so that a wheel goes to a window that can animate its scroll; a window that stays frozen is the runner problem, and
	/// no wheel is sent.
	@MainActor
	private func awaitDrawing(before what: String) throws {
		let start = ProcessInfo.processInfo.systemUptime
		let drew = poll(timeout: 10) {
			traceLines(since: start).contains { $0.hasPrefix("drawing: ") && !$0.hasPrefix("drawing: 0 ") }
		}
		guard drew else {
			throw RunnerProblem(description: "Runner problem, not the transcript: the window drew no frame for 10 s before \(what), so no wheel was sent (\(traceLines(since: start).filter { $0.hasPrefix("drawing: ") }.suffix(8).joined(separator: "; ")))")
		}
	}

	/// The frame counts the trace noted for each wheel event since `start` ("4 in 0.5 s, window visible true …").
	private func wheelFrames(since start: TimeInterval) -> [String] {
		traceLines(since: start).compactMap { line in
			line.hasPrefix("frames after the wheel: ") ? String(line.dropFirst("frames after the wheel: ".count)) : nil
		}
	}

	/// The app's wheel trace from `start` on, each line without its time stamp. The app stamps lines with the system's
	/// uptime, the clock this process reads too.
	private func traceLines(since start: TimeInterval) -> [String] {
		let trace = wheelTrace.flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? ""
		return trace.split(separator: "\n").compactMap { line in
			let parts = line.split(separator: " ", maxSplits: 1)
			guard parts.count == 2, let stamp = TimeInterval(parts[0]), stamp >= start else { return nil }
			return String(parts[1])
		}
	}

	/// What a failed wheel assertion needs: where the scroll was aimed, the app's own wheel and scrolling decisions
	/// (`DEVORA_WHEEL_TRACE`), the transcript's accessibility tree and a screenshot.
	@MainActor
	private func attachWheelEvidence(_ app: XCUIApplication, _ transcript: XCUIElement, _ what: String, _ aim: String) {
		let aimed = XCTAttachment(string: aim)
		aimed.name = "Where \(what) was aimed"
		aimed.lifetime = .keepAlways
		add(aimed)
		let trace = XCTAttachment(string: (wheelTrace.flatMap { try? String(contentsOf: $0, encoding: .utf8) }) ?? "(no wheel trace)")
		trace.name = "The app's wheel decisions"
		trace.lifetime = .keepAlways
		add(trace)
		attachTree(of: transcript)
		capture(app, "After \(what)")
	}

	/// The transcript's accessibility tree, as XCUITest sees it: what a failure about where its rows are needs.
	@MainActor
	private func attachTree(of transcript: XCUIElement) {
		let dump = XCTAttachment(string: transcript.debugDescription)
		dump.name = "Transcript accessibility tree"
		dump.lifetime = .keepAlways
		add(dump)
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

	/// The turns in view, top to bottom, as `<number>@<top>`: what a failure shows.
	var inViewSummary: String { turnsInView.sorted { $0.top < $1.top }.map { "\($0.number)@\(Int($0.top))" }.joined(separator: ", ") }
}
