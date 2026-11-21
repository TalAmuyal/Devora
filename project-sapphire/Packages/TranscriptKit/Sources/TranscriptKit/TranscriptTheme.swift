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

extension EnvironmentValues {
	@Entry var transcriptTheme = TranscriptTheme()
}
