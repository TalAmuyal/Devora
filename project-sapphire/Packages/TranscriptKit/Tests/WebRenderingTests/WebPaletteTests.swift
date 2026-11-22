import AppKit
import Foundation
import Testing

@testable import WebRendering

/// The web renderer's colors are the system's in each appearance, so the panes and the diagrams read as part of the app.
@MainActor
struct WebPaletteTests {
	private func resolved(_ color: NSColor, in name: NSAppearance.Name, page: NSColor) -> String {
		var hex = ""
		NSAppearance(named: name)!.performAsCurrentDrawingAppearance { hex = WebPalette.hex(color, over: page) }
		return hex
	}

	@Test func eachAppearanceGivesTheSystemsColorsAndScheme() {
		let light = WebPalette.variables(for: NSAppearance(named: .aqua)!, page: .textBackgroundColor)
		let dark = WebPalette.variables(for: NSAppearance(named: .darkAqua)!, page: .textBackgroundColor)
		#expect(light["--color-scheme"] == "light")
		#expect(dark["--color-scheme"] == "dark")
		for (variables, name) in [(light, NSAppearance.Name.aqua), (dark, .darkAqua)] {
			#expect(variables["--color-bg"] == resolved(.textBackgroundColor, in: name, page: .textBackgroundColor))
			#expect(variables["--color-fg"] == resolved(.labelColor, in: name, page: .textBackgroundColor))
			#expect(variables["--color-accent"] == resolved(.controlAccentColor, in: name, page: .textBackgroundColor))
		}
		#expect(light["--color-fg"] != dark["--color-fg"] && light["--color-bg"] != dark["--color-bg"])
		let contrast = WebPalette.variables(for: NSAppearance(named: .accessibilityHighContrastDarkAqua)!, page: .textBackgroundColor)
		#expect(contrast["--color-scheme"] == "dark")
	}

	/// The page is the caller's: the transcript's diagrams sit on the window's background, a pane's text on a document's.
	@Test func thePageIsTheCallers() {
		let aqua = NSAppearance(named: .aqua)!
		#expect(
			WebPalette.variables(for: aqua, page: .windowBackgroundColor)["--color-bg"]
				== resolved(.windowBackgroundColor, in: .aqua, page: .windowBackgroundColor))
		#expect(
			WebPalette.variables(for: aqua, page: .textBackgroundColor)["--color-bg"]
				== resolved(.textBackgroundColor, in: .aqua, page: .textBackgroundColor))
	}

	/// Mermaid derives its shades from the variables, so a translucent system color is given as it shows on the page.
	@Test func aTranslucentColorIsFlattenedOntoThePage() {
		let half = NSColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.5)
		#expect(WebPalette.hex(half, over: NSColor(srgbRed: 0, green: 0, blue: 0, alpha: 1)) == "#808080")
	}

	/// A color the renderer reads that the palette doesn't set falls back to Ember's dark theme: a dark patch on a light
	/// page. Every `var(--…)` in its stylesheets and every `cssVar('--…')` in its script is the palette's, or defined by
	/// the renderer itself (`:root` in its stylesheet: fonts, sizes, spacing).
	@Test func everyVariableTheRendererReadsIsProvided() throws {
		let source = URL(filePath: #filePath).deletingLastPathComponent()
			.appending(path: "../../../../../web-renderer/src", directoryHint: .isDirectory).standardized
		let files = try FileManager.default.contentsOfDirectory(at: source, includingPropertiesForKeys: nil)
			.filter { ["css", "ts"].contains($0.pathExtension) }
		#expect(files.contains { $0.lastPathComponent == "content.css" }, "the renderer's sources at \(source.path)")
		var read = Set<String>(), defined = Set<String>()
		for file in files {
			let text = try String(contentsOf: file, encoding: .utf8)
			for match in text.matches(of: /(?:var\(|cssVar\(')(--[a-z0-9-]+)/) { read.insert(String(match.1)) }
			for match in text.matches(of: /(--[a-z0-9-]+)\s*:/) { defined.insert(String(match.1)) }
		}
		let provided = Set(WebPalette.variables(for: NSAppearance(named: .aqua)!, page: .textBackgroundColor).keys)
		#expect(!read.isEmpty)
		#expect(read.subtracting(defined).subtracting(provided).sorted() == [], "read by the renderer, set by no one")
	}
}
