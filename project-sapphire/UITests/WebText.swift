import XCTest

extension XCUIElement {
	/// Waits, as `waitForExistence` does for an element, until this element's accessibility tree shows `text`, and records a failure listing the texts it did show when it doesn't.
	///
	/// A web view's text isn't in the same attribute on every macOS: a heading's text is its `value` on older ones, while newer ones give it its level as a number in `value` and its text in `title`, which makes a `value CONTAINS` predicate throw.
	/// So this reads one snapshot of the tree at a time and matches a string `value`, the `title` or the `label`.
	@MainActor
	@discardableResult
	func waitForText(_ text: String, timeout: TimeInterval, _ message: String = "", file: StaticString = #filePath, line: UInt = #line) -> Bool {
		let deadline = Date.now.addingTimeInterval(timeout)
		var seen: [String] = []
		repeat {
			seen = (try? snapshot()).map { Self.texts(of: $0) } ?? []
			if seen.contains(where: { $0.contains(text) }) { return true }
			Thread.sleep(forTimeInterval: 0.2)
		} while Date.now < deadline
		let shown = seen.isEmpty ? "nothing" : seen.prefix(40).map { "\"\($0)\"" }.joined(separator: ", ") + (seen.count > 40 ? ", …" : "")
		XCTFail("\(message.isEmpty ? "" : "\(message): ")\"\(text)\" didn't show within \(timeout) s; shown: \(shown)", file: file, line: line)
		return false
	}

	private static func texts(of root: XCUIElementSnapshot) -> [String] {
		var texts: [String] = []
		func walk(_ element: XCUIElementSnapshot) {
			for text in [element.value as? String, element.title, element.label] {
				if let text, !text.isEmpty, !texts.contains(text) { texts.append(text) }
			}
			element.children.forEach(walk)
		}
		walk(root)
		return texts
	}
}
