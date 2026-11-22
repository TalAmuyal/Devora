import AppKit
import SwiftUI

/// The transcript's colors: the system's semantic colors, which follow the view's appearance (light or dark, and
/// increased contrast) and the user's accent color by themselves.
struct TranscriptTheme: Equatable {
	var text: Color = .primary
	/// Collapsed summaries, quotes and other quieter text.
	var secondary: Color = .secondary
	var heading: Color = .primary
	var link: Color = .accentColor
	var inlineCode: Color = Color(nsColor: .quaternarySystemFill)
	var codeBlock: Color = Color(nsColor: .tertiarySystemFill)
	var quoteEdge: Color = .accentColor
	var border: Color = Color(nsColor: .separatorColor)
	var error: Color = .red
}

extension TranscriptTheme {
	/// On the user's blue bubble: white text, with its code and quotes in translucent white and black.
	static let onBubble = TranscriptTheme(
		text: .white, secondary: .white.opacity(0.8), heading: .white, link: .white, inlineCode: .white.opacity(0.2),
		codeBlock: .black.opacity(0.18), quoteEdge: .white, border: .white.opacity(0.35), error: .white)
}

extension EnvironmentValues {
	@Entry var transcriptTheme = TranscriptTheme()
}
