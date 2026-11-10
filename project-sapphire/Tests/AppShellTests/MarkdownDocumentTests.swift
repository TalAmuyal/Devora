import Foundation
import Testing

@testable import AppShell

struct MarkdownDocumentTests {
	@Test func blocksKeepTheirStructure() {
		let document = MarkdownDocument("""
			## Results

			Some **bold**, _emphasis_, `code`, ~~strike~~ and [a link](https://example.com).

			3. three
			   - nested
			4. four

			> quoted

			---
			""")
		#expect(document.blocks == [
			.heading(level: 2, [InlineRun("Results")]),
			.paragraph([
				InlineRun("Some "), InlineRun("bold", style: .strong), InlineRun(", "), InlineRun("emphasis", style: .emphasis),
				InlineRun(", "), InlineRun("code", style: .code), InlineRun(", "), InlineRun("strike", style: .strikethrough),
				InlineRun(" and "), InlineRun("a link", link: URL(string: "https://example.com")), InlineRun("."),
			]),
			.list(start: 3, items: [
				[.paragraph([InlineRun("three")]), .list(start: nil, items: [[.paragraph([InlineRun("nested")])]])],
				[.paragraph([InlineRun("four")])],
			]),
			.quote([.paragraph([InlineRun("quoted")])]),
			.thematicBreak,
		])
	}

	@Test func tablesKeepTheirColumnAlignment() {
		let document = MarkdownDocument("""
			| Name | Rows | Time |
			| :--- | :---: | ---: |
			| web | 2,000 | 120 ms |
			""")
		#expect(document.blocks == [
			.table(
				alignments: [.leading, .center, .trailing],
				header: [[InlineRun("Name")], [InlineRun("Rows")], [InlineRun("Time")]],
				rows: [[[InlineRun("web")], [InlineRun("2,000")], [InlineRun("120 ms")]]]),
		])
	}

	@Test func codeBlocksKeepTheirLanguageAndText() {
		let document = MarkdownDocument("```swift\nlet a = 1\nlet b = 2\n```\n\n```\nplain\n```")
		#expect(document.blocks == [.code(language: "swift", "let a = 1\nlet b = 2"), .code(language: nil, "plain")])
	}

	@Test func nestedStylesCombineAndBreaksStay() {
		let document = MarkdownDocument("**bold _both_**  \nnext\nsame line")
		#expect(document.blocks == [
			.paragraph([
				InlineRun("bold ", style: .strong), InlineRun("both", style: [.strong, .emphasis]),
				InlineRun("\nnext same line"),
			]),
		])
	}

	@Test func syntaxItDoesNotModelKeepsItsSource() {
		let document = MarkdownDocument("<div>raw</div>\n\nAn ![image](x.png) and <b>html</b>.")
		#expect(document.blocks == [
			.paragraph([InlineRun("<div>raw</div>")]),
			.paragraph([InlineRun("An image and <b>html</b>.")]),
		])
	}
}
