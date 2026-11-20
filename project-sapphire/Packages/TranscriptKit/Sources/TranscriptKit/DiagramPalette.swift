import AppKit

/// The Mermaid diagrams' colors: the web renderer's CSS variables, from the system's colors in an appearance, so that a
/// diagram reads as part of the transcript in light, dark and increased contrast, with the user's accent color.
enum DiagramPalette {
	static func variables(for appearance: NSAppearance) -> [String: String] {
		var variables: [String: String] = [:]
		appearance.performAsCurrentDrawingAppearance {
			let page = NSColor.windowBackgroundColor
			// Mermaid derives its shades from these, so each is flattened onto the page as an opaque color.
			func css(_ color: NSColor) -> String { hex(color, over: page) }
			let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
			variables = [
				"--color-scheme": dark ? "dark" : "light",
				"--font-ui": "-apple-system, system-ui, sans-serif",
				"--color-bg": css(page),
				"--color-text": css(.labelColor),
				"--color-subtext0": css(.secondaryLabelColor),
				"--color-accent": css(.controlAccentColor),
				"--color-surface0": css(.controlBackgroundColor),
				"--color-surface1": css(.quaternarySystemFill),
				"--color-surface2": css(.tertiarySystemFill),
			]
		}
		return variables
	}

	/// `#rrggbb` for `color` as it shows over `page`, both in the current drawing appearance.
	static func hex(_ color: NSColor, over page: NSColor) -> String {
		guard let front = color.usingColorSpace(.sRGB), let back = page.usingColorSpace(.sRGB) else { return "#000000" }
		let alpha = front.alphaComponent
		func channel(_ front: CGFloat, _ back: CGFloat) -> Int { Int(((front * alpha + back * (1 - alpha)) * 255).rounded()) }
		return String(
			format: "#%02X%02X%02X", channel(front.redComponent, back.redComponent), channel(front.greenComponent, back.greenComponent),
			channel(front.blueComponent, back.blueComponent))
	}
}
