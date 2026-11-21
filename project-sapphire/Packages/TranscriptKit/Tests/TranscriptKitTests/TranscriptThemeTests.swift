import AppKit
import Testing

@testable import TranscriptKit

/// The diagrams' colors are the system's in each appearance, so a diagram reads as part of the transcript.
@MainActor
struct DiagramPaletteTests {
	private func resolved(_ color: NSColor, in name: NSAppearance.Name) -> String {
		var hex = ""
		NSAppearance(named: name)!.performAsCurrentDrawingAppearance { hex = DiagramPalette.hex(color, over: .windowBackgroundColor) }
		return hex
	}

	@Test func eachAppearanceGivesTheSystemsColorsAndScheme() {
		let light = DiagramPalette.variables(for: NSAppearance(named: .aqua)!)
		let dark = DiagramPalette.variables(for: NSAppearance(named: .darkAqua)!)
		#expect(light["--color-scheme"] == "light")
		#expect(dark["--color-scheme"] == "dark")
		for (variables, name) in [(light, NSAppearance.Name.aqua), (dark, .darkAqua)] {
			#expect(variables["--color-text"] == resolved(.labelColor, in: name))
			#expect(variables["--color-bg"] == resolved(.windowBackgroundColor, in: name))
			#expect(variables["--color-accent"] == resolved(.controlAccentColor, in: name))
		}
		#expect(light["--color-text"] != dark["--color-text"] && light["--color-bg"] != dark["--color-bg"])
		let contrast = DiagramPalette.variables(for: NSAppearance(named: .accessibilityHighContrastDarkAqua)!)
		#expect(contrast["--color-scheme"] == "dark")
	}

	/// Mermaid derives its shades from the variables, so a translucent system color is given as it shows on the page.
	@Test func aTranslucentColorIsFlattenedOntoThePage() {
		let half = NSColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.5)
		#expect(DiagramPalette.hex(half, over: NSColor(srgbRed: 0, green: 0, blue: 0, alpha: 1)) == "#808080")
		#expect(DiagramPalette.hex(.white.usingColorSpace(.sRGB)!, over: .black.usingColorSpace(.sRGB)!) == "#FFFFFF")
	}
}
