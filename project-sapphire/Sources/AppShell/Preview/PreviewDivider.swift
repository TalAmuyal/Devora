import AppKit
import SwiftUI

/// A drag only reports its transient width; the core's resize command runs once, when it ends.
struct PreviewDivider: View {
	let pane: UInt64
	let width: CGFloat
	let minimum: CGFloat
	let maximum: CGFloat
	let drag: (CGFloat) -> Void
	let commit: (UInt32) -> Void
	@State private var initial: (width: CGFloat, bounds: ClosedRange<CGFloat>)?
	@State private var hovering = false

	var body: some View {
		ZStack {
			Color(nsColor: .windowBackgroundColor)
			Rectangle().fill(Color(nsColor: .separatorColor)).frame(width: 1)
		}
		.contentShape(Rectangle())
		// Win over the row's horizontal scrolling while the pointer is on a divider.
		.highPriorityGesture(DragGesture(minimumDistance: 0, coordinateSpace: .global)
			.onChanged { value in
				if initial == nil { initial = (width, minimum...max(minimum, maximum)) }
				if let next = dragged(by: value.translation.width) { drag(next) }
			}
			.onEnded { value in
				if let next = dragged(by: value.translation.width) { commit(UInt32(next)) }
				initial = nil
			})
		.onHover { inside in
			guard inside != hovering else { return }
			hovering = inside
			if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
		}
		.onDisappear { if hovering { NSCursor.pop(); hovering = false } }
		.accessibilityElement(children: .ignore)
		.accessibilityLabel("Resize preview")
		.accessibilityIdentifier("preview-divider-\(pane)")
		.accessibilityValue("\(Int(width.rounded())) points")
		.accessibilityAdjustableAction { direction in
			let delta: CGFloat = direction == .increment ? 10 : -10
			let next = UInt32(min(maximum, max(minimum, width + delta)).rounded())
			if next != UInt32(width.rounded()) { commit(next) }
		}
	}

	private func dragged(by translation: CGFloat) -> CGFloat? {
		guard let initial else { return nil }
		return min(initial.bounds.upperBound, max(initial.bounds.lowerBound, initial.width - translation)).rounded()
	}
}
