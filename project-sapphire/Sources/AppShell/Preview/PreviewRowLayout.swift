import CoreBindings
import Foundation

/// Projects durable preferred widths into a viewport without changing core state on window resize.
struct PreviewRowLayout {
	static let divider: CGFloat = 6
	let terminal: CGFloat
	let previews: [CGFloat]
	let content: CGFloat

	init(available: CGFloat, preferred: [CGFloat], rules: PreviewLayoutRules) {
		let minimum = CGFloat(rules.minimumPreviewWidth)
		let terminalMinimum = CGFloat(rules.minimumTerminalWidth)
		let dividers = CGFloat(preferred.count) * Self.divider
		content = max(available, terminalMinimum + CGFloat(preferred.count) * minimum + dividers)
		let excess = preferred.reduce(0) { $0 + max(0, $1 - minimum) }
		let budget = content - terminalMinimum - dividers - CGFloat(preferred.count) * minimum
		let scale = excess > 0 ? min(1, max(0, budget / excess)) : 0
		previews = preferred.map { minimum + max(0, $0 - minimum) * scale }
		terminal = max(terminalMinimum, content - dividers - previews.reduce(0, +))
	}
}
