import SwiftUI

/// The web transcript's marks of a row, until chat bubbles replace them: a dot in the kind's color at the row's left,
/// and an amber edge for an interrupted or failed item. Kept apart so that the look can be swapped in one place.
struct KindMarker: ViewModifier {
	let kind: TranscriptRowModel.Kind
	let unfinished: Bool

	func body(content: Content) -> some View {
		content
			.padding(.leading, 22)
			.overlay(alignment: .topLeading) {
				Circle().fill(color).frame(width: 8, height: 8).padding(.top, 6).accessibilityHidden(true)
			}
			.overlay(alignment: .leading) {
				if unfinished { Rectangle().fill(Self.amber).frame(width: 2).accessibilityHidden(true) }
			}
	}

	private static let amber = Color(red: 0.96, green: 0.62, blue: 0.04)

	private var color: Color {
		switch kind {
		case .user: Color(red: 0.38, green: 0.65, blue: 0.98)
		case .assistant: Color(red: 0.29, green: 0.87, blue: 0.5)
		case .tool: Color(red: 0.61, green: 0.64, blue: 0.69)
		case .thinking: Color(red: 0.75, green: 0.52, blue: 0.99)
		case .system: Self.amber
		}
	}
}

extension View {
	func kindMarker(_ kind: TranscriptRowModel.Kind, unfinished: Bool) -> some View {
		modifier(KindMarker(kind: kind, unfinished: unfinished))
	}
}
