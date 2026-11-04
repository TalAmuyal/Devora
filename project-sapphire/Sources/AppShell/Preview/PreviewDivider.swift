import AppKit
import SwiftUI

/// Pointer tracking is transient; every size change goes through the core's resize command.
struct PreviewDivider: View {
	let pane: UInt64
	let width: CGFloat
	let minimum: CGFloat
	let maximum: CGFloat
	let resize: (UInt32) -> Void
	@State private var initial: (width: CGFloat, bounds: ClosedRange<CGFloat>)?
	@State private var lastSent: UInt32?
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
				if initial == nil { initial = (width, minimum...max(minimum, maximum)); lastSent = UInt32(width.rounded()) }
				update(translation: value.translation.width)
			}
			.onEnded { value in
				update(translation: value.translation.width)
				initial = nil
				lastSent = nil
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
			if next != UInt32(width.rounded()) { resize(next) }
		}
	}

	private func update(translation: CGFloat) {
		guard let initial else { return }
		let next = UInt32(min(initial.bounds.upperBound, max(initial.bounds.lowerBound, initial.width - translation)).rounded())
		if next != lastSent { lastSent = next; resize(next) }
	}
}
