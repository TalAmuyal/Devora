import XCTest

extension XCUIElement {
	/// Waits until one snapshot of this element's accessibility tree satisfies `condition`, taking a new snapshot until it does, and records a failure when it doesn't within `timeout`.
	///
	/// Every query resolves through a snapshot of the tree, so checking several things in one snapshot costs one look rather than one per query, and no waiter interrupts a query midway.
	/// The failure says what the last snapshot showed (`describe`), or that none could be taken, without another snapshot.
	@MainActor
	@discardableResult
	func waitForSnapshot(timeout: TimeInterval, _ message: String, file: StaticString = #filePath, line: UInt = #line,
		describe: (XCUIElementSnapshot) -> String, _ condition: (XCUIElementSnapshot) -> Bool) -> Bool {
		let deadline = Date.now.addingTimeInterval(timeout)
		var last: XCUIElementSnapshot?
		var failure: Error?
		repeat {
			do {
				let snapshot = try snapshot()
				if condition(snapshot) { return true }
				last = snapshot
			} catch {
				failure = error
			}
			Thread.sleep(forTimeInterval: 0.2)
		} while Date.now < deadline
		let shown = last.map(describe) ?? "no snapshot could be taken (\(failure.map { "\($0)" } ?? "no element"))"
		XCTFail("\(message) (not within \(timeout) s): \(shown)", file: file, line: line)
		return false
	}

	/// Waits, as `waitForExistence` does for an element, until this element's accessibility tree shows `text`, and records a failure listing the texts it did show when it doesn't.
	///
	/// A web view's text isn't in the same attribute on every macOS: a heading's text is its `value` on older ones, while newer ones give it its level as a number in `value` and its text in `title`, which makes a `value CONTAINS` predicate throw.
	/// So this matches a string `value`, the `title` or the `label`.
	@MainActor
	@discardableResult
	func waitForText(_ text: String, timeout: TimeInterval, _ message: String = "", file: StaticString = #filePath, line: UInt = #line) -> Bool {
		waitForSnapshot(timeout: timeout, "\(message.isEmpty ? "" : "\(message): ")\"\(text)\" didn't show", file: file, line: line, describe: { snapshot in
			let seen = snapshot.texts
			return "shown: " + (seen.isEmpty ? "nothing" : seen.prefix(40).map { "\"\($0)\"" }.joined(separator: ", ") + (seen.count > 40 ? ", …" : ""))
		}) { snapshot in
			snapshot.texts.contains { $0.contains(text) }
		}
	}
}

extension XCUIElementSnapshot {
	/// This element and everything under it, depth first.
	var descendants: [XCUIElementSnapshot] {
		[self] + children.flatMap(\.descendants)
	}

	/// The distinct texts in the tree: each element's string `value`, `title` and `label`.
	var texts: [String] {
		var texts: [String] = []
		for element in descendants {
			for text in [element.value as? String, element.title, element.label] {
				if let text, !text.isEmpty, !texts.contains(text) { texts.append(text) }
			}
		}
		return texts
	}
}
