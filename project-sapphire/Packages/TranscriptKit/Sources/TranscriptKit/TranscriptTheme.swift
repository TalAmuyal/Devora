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
	/// Whether a link is underlined: in the user's bubbles, where it has the text's color.
	var underlinesLinks = false
}

extension TranscriptTheme {
	/// On the user's blue bubble: white text, with its code and quotes in translucent white and black.
	static let onBubble = TranscriptTheme(
		text: .white, secondary: .white.opacity(0.8), heading: .white, link: .white, inlineCode: .white.opacity(0.2),
		codeBlock: .black.opacity(0.18), quoteEdge: .white, border: .white.opacity(0.35), error: .white,
		underlinesLinks: true)

	/// In an outline-only bubble (a prompt the agent doesn't have): its text, headings, links and quotes in the
	/// outline's `tint`, on the page's own code fills.
	static func outlined(_ tint: Color) -> TranscriptTheme {
		TranscriptTheme(
			text: tint, secondary: tint.opacity(0.8), heading: tint, link: tint, quoteEdge: tint, error: tint, underlinesLinks: true)
	}
}

extension EnvironmentValues {
	@Entry var transcriptTheme = TranscriptTheme()
}
