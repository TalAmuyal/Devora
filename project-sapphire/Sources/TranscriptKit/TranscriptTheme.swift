import AppKit
import SwiftUI

/// The transcript's colors, from the Devora theme's CSS variables as the web renderer uses them for Markdown, each
/// falling back to the system's when the theme doesn't set it.
struct TranscriptTheme: Equatable {
	/// The page; nil draws none, so the window's background shows.
	var background: NSColor?
	var text: Color = .primary
	/// Collapsed summaries, quotes and other quieter text.
	var secondary: Color = .secondary
	var heading: Color = .primary
	var link: Color = .accentColor
	var inlineCode: Color = .secondary.opacity(0.15)
	var codeBlock: Color = Color(nsColor: .quaternaryLabelColor).opacity(0.5)
	var quoteEdge: Color = .secondary
	var border: Color = Color(nsColor: .separatorColor)
	var error: Color = .red

	init() {}

	init(_ variables: [String: String]) {
		func color(_ name: String) -> NSColor? { variables[name].flatMap(NSColor.init(css:)) }
		background = color("--color-bg")
		if let fg = color("--color-fg") { text = Color(nsColor: fg) }
		if let quieter = color("--color-fg-secondary") ?? color("--color-subtext0") { secondary = Color(nsColor: quieter) }
		if let accent = color("--color-accent") {
			heading = Color(nsColor: accent)
			quoteEdge = Color(nsColor: accent)
		}
		if let blue = color("--color-blue") { link = Color(nsColor: blue) }
		if let secondaryBackground = color("--color-bg-secondary") { codeBlock = Color(nsColor: secondaryBackground) }
		if let line = color("--color-border") { border = Color(nsColor: line) }
		if let failure = color("--color-status-error") { error = Color(nsColor: failure) }
	}
}

extension EnvironmentValues {
	@Entry var transcriptTheme = TranscriptTheme()
}

extension NSColor {
	/// Whether the color is dark, by its relative luminance.
	var isDark: Bool {
		guard let rgb = usingColorSpace(.sRGB) else { return false }
		return 0.2126 * rgb.redComponent + 0.7152 * rgb.greenComponent + 0.0722 * rgb.blueComponent < 0.5
	}

	/// A CSS hex color, `#rgb`, `#rrggbb` or `#rrggbbaa`, as the theme files write them; nil for anything else.
	convenience init?(css: String) {
		var hex = css.trimmingCharacters(in: .whitespaces)
		guard hex.hasPrefix("#") else { return nil }
		hex.removeFirst()
		if hex.count == 3 { hex = hex.map { "\($0)\($0)" }.joined() }
		guard hex.count == 6 || hex.count == 8, let value = UInt64(hex, radix: 16) else { return nil }
		let rgba = hex.count == 6 ? value << 8 | 0xFF : value
		self.init(
			srgbRed: CGFloat(rgba >> 24 & 0xFF) / 255, green: CGFloat(rgba >> 16 & 0xFF) / 255,
			blue: CGFloat(rgba >> 8 & 0xFF) / 255, alpha: CGFloat(rgba & 0xFF) / 255)
	}
}
