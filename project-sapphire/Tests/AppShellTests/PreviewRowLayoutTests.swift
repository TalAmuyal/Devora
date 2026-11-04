import CoreBindings
import Foundation
import Testing
@testable import AppShell

struct PreviewRowLayoutTests {
	private let rules = PreviewLayoutRules(minimumPreviewWidth: 200, minimumTerminalWidth: 240, maximumPreviewWidth: 16_384)

	@Test func narrowWindowsKeepEveryPaneReachableWithoutShrinkingBelowItsMinimum() {
		let row = PreviewRowLayout(available: 500, preferred: [500, 320], rules: rules)
		#expect(row.content == 652)
		#expect(row.terminal == 240)
		#expect(row.previews == [200, 200])
	}

	@Test func aWindowResizeConstrainsButDoesNotOverwritePreferredWidths() {
		let preferred: [CGFloat] = [500, 320]
		let small = PreviewRowLayout(available: 800, preferred: preferred, rules: rules)
		#expect(small.terminal >= 240 && small.previews.allSatisfy { $0 >= 200 })
		let large = PreviewRowLayout(available: 1200, preferred: preferred, rules: rules)
		#expect(large.previews == preferred)
		#expect(large.terminal == 368)
	}
}
