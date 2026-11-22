import AppKit

/// The web renderer's colors (its CSS variables) for an appearance, from the system's: the panes' Markdown (previews,
/// the User Guide) and every Mermaid diagram, the transcript's included, read as part of the app in light, dark and
/// increased contrast, with the user's accent color. Every color variable the renderer reads is here
/// (`WebPaletteTests`): one it doesn't set falls back to Ember's dark theme.
public enum WebPalette {
	/// `page` is what the content sits on: the window's background for the transcript, a document's for a pane.
	public static func variables(for appearance: NSAppearance, page: NSColor) -> [String: String] {
		var variables: [String: String] = [:]
		appearance.performAsCurrentDrawingAppearance {
			// Mermaid derives its shades from these, so each is flattened onto the page as an opaque color.
			func css(_ color: NSColor) -> String { hex(color, over: page) }
			let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
			variables = [
				"--color-scheme": dark ? "dark" : "light",
				"--font-ui": "-apple-system, system-ui, sans-serif",
				"--color-bg": css(page),
				"--color-fg": css(.labelColor),
				"--color-text": css(.labelColor),
				"--color-fg-secondary": css(.secondaryLabelColor),
				"--color-subtext0": css(.secondaryLabelColor),
				"--color-accent": css(.controlAccentColor),
				"--color-blue": css(.linkColor),
				"--color-bg-secondary": css(.tertiarySystemFill),
				"--color-bg-overlay": css(.quaternarySystemFill),
				"--color-border": css(.separatorColor),
				"--color-status-error": css(.systemRed),
				"--color-surface0": css(.controlBackgroundColor),
				"--color-surface1": css(.quaternarySystemFill),
				"--color-surface2": css(.tertiarySystemFill),
			]
		}
		return variables
	}

	/// `#rrggbb` for `color` as it shows over `page`, both in the current drawing appearance.
	public static func hex(_ color: NSColor, over page: NSColor) -> String {
		guard let front = color.usingColorSpace(.sRGB), let back = page.usingColorSpace(.sRGB) else { return "#000000" }
		let alpha = front.alphaComponent
		func channel(_ front: CGFloat, _ back: CGFloat) -> Int { Int(((front * alpha + back * (1 - alpha)) * 255).rounded()) }
		return String(
			format: "#%02X%02X%02X", channel(front.redComponent, back.redComponent), channel(front.greenComponent, back.greenComponent),
			channel(front.blueComponent, back.blueComponent))
	}
}

/// Calls back when a view's colors may have changed: its effective appearance (the app's setting, or the system's on
/// Automatic), or the system's colors, such as the accent, which can change with no change of appearance. One place for
/// every view that draws in `WebPalette`'s colors.
@MainActor
public final class PaletteFollower {
	private var observation: NSKeyValueObservation?
	private var colors: NSObjectProtocol?

	public init(_ view: NSView, onChange: @escaping @MainActor () -> Void) {
		observation = view.observe(\.effectiveAppearance) { _, _ in MainActor.assumeIsolated { onChange() } }
		colors = NotificationCenter.default.addObserver(forName: NSColor.systemColorsDidChangeNotification, object: nil, queue: .main) { _ in
			MainActor.assumeIsolated { onChange() }
		}
	}

	isolated deinit {
		if let colors { NotificationCenter.default.removeObserver(colors) }
	}
}
