import AppKit
import CoreBindings
import SwiftUI
import Testing

@testable import AppShell

/// Colors from Catppuccin Macchiato, Devora's default theme.
private let macchiato = [
	"--color-bg": "#24273A", "--color-fg": "#CAD3F5", "--color-fg-secondary": "#A5ADCB", "--color-accent": "#C6A0F6",
	"--color-blue": "#8AADF4", "--color-bg-secondary": "#1E2030", "--color-border": "#494D64", "--color-status-error": "#ED8796",
]

private func color(_ css: String) -> Color { Color(nsColor: NSColor(css: css)!) }

struct TranscriptThemeTests {
	@Test func aCSSHexColorIsReadInEachForm() throws {
		let full = try #require(NSColor(css: "#24273A")?.usingColorSpace(.sRGB))
		#expect(Int((full.redComponent * 255).rounded()) == 0x24 && Int((full.blueComponent * 255).rounded()) == 0x3A && full.alphaComponent == 1)
		#expect(NSColor(css: "#fff")?.usingColorSpace(.sRGB)?.greenComponent == 1)
		#expect(NSColor(css: " #00000080 ")?.alphaComponent ?? 0 > 0.49)
		for invalid in ["24273A", "#12345", "#GGGGGG", "rgb(1, 2, 3)", ""] { #expect(NSColor(css: invalid) == nil, "\(invalid)") }
	}

	@Test func theThemesColorsAreTheWebRenderersMarkdownColors() {
		let theme = TranscriptTheme(macchiato)
		#expect(theme.background == NSColor(css: "#24273A"))
		#expect(theme.text == color("#CAD3F5"))
		#expect(theme.secondary == color("#A5ADCB"))
		#expect(theme.heading == color("#C6A0F6") && theme.quoteEdge == color("#C6A0F6"))
		#expect(theme.link == color("#8AADF4"))
		#expect(theme.codeBlock == color("#1E2030"))
		#expect(theme.border == color("#494D64"))
		#expect(theme.error == color("#ED8796"))
	}

	@Test func whatTheThemeDoesntSetIsTheSystems() {
		let system = TranscriptTheme()
		let partial = TranscriptTheme(["--color-fg": "#CAD3F5", "--color-accent": "not a color"])
		#expect(partial.text == color("#CAD3F5"))
		#expect(partial.background == nil && partial.heading == system.heading && partial.link == system.link)
		#expect(TranscriptTheme([:]) == system)
		// Without --color-fg-secondary, quieter text is the web summaries' --color-subtext0.
		#expect(TranscriptTheme(["--color-subtext0": "#A5ADCB"]).secondary == color("#A5ADCB"))
	}
}

@MainActor
struct TranscriptThemedViewTests {
	private func shown(_ theme: [String: String]) -> (NSWindow, TranscriptScrollView) {
		let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
		window.isReleasedWhenClosed = false
		let view = TranscriptScrollView(anchor: nil)
		view.hasVerticalScroller = false
		view.frame = window.contentView!.bounds
		window.contentView!.addSubview(view)
		window.layoutIfNeeded()
		view.diagramTheme = theme
		let items = (1...3).map {
			TranscriptItem(sequence: UInt64($0), cliSession: "s", parentToolUseId: nil, state: .complete, content: .assistant(text: "# Reply \($0)"))
		}
		view.apply(items, questions: [], generation: 1)
		view.layoutSubtreeIfNeeded()
		return (window, view)
	}

	private func shownThemes(_ view: TranscriptScrollView) -> [TranscriptTheme?] {
		(0..<view.table.numberOfRows).map { (view.table.view(atColumn: 0, row: $0, makeIfNecessary: false) as? TranscriptCell)?.entry?.theme }
	}

	@Test func theThemeReachesTheRowsAndThePage() {
		let (window, view) = shown(macchiato)
		defer { window.close() }
		#expect(shownThemes(view) == Array(repeating: TranscriptTheme(macchiato), count: 3))
		#expect(view.drawsBackground && view.backgroundColor == NSColor(css: "#24273A"))
		#expect(view.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua, "A dark page draws the system's colors for dark")
	}

	@Test func aNewThemeReachesTheRowsShowing() {
		let (window, view) = shown([:])
		defer { window.close() }
		#expect(!view.drawsBackground, "No theme: the window's background")
		let light = ["--color-bg": "#EFF1F5", "--color-fg": "#4C4F69"]
		view.diagramTheme = light
		view.layoutSubtreeIfNeeded()
		#expect(shownThemes(view) == Array(repeating: TranscriptTheme(light), count: 3))
		#expect(view.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua)
	}
}
