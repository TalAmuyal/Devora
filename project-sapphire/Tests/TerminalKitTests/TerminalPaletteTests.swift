import Foundation
import Testing
@testable import TerminalKit

/// Every palette a user can pick stays readable: its text colors read at 4.5:1 (WCAG AA) on its page, in both
/// variants. The exempt colors are background roles, which programs fill behind other text rather than write in:
/// "black" on a dark page, and "white" and "bright white" on a light one, where "black" is exempt too: Selenized and
/// Gruvbox, as their authors drew them, make it one of their light page's own shades.
struct TerminalPaletteTests {
	private static let names = [
		"black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
		"bright black", "bright red", "bright green", "bright yellow", "bright blue", "bright magenta", "bright cyan", "bright white",
	]

	private func luminance(_ hex: String) -> Double {
		let value = Int(hex.dropFirst(), radix: 16)!
		func channel(_ byte: Int) -> Double {
			let c = Double(byte) / 255
			return c <= 0.03928 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4)
		}
		return 0.2126 * channel(value >> 16 & 0xFF) + 0.7152 * channel(value >> 8 & 0xFF) + 0.0722 * channel(value & 0xFF)
	}

	private func contrast(_ a: String, _ b: String) -> Double {
		let (x, y) = (luminance(a), luminance(b))
		return (max(x, y) + 0.05) / (min(x, y) + 0.05)
	}

	@Test(arguments: TerminalPalette.Family.allCases)
	func everyTextColorReadsOnItsPage(_ family: TerminalPalette.Family) {
		for (variant, palette, backgroundRoles) in [
			("light", family.light, ["black", "white", "bright white"]), ("dark", family.dark, ["black"]),
		] {
			#expect(palette.ansi.count == 16)
			#expect(contrast(palette.foreground, palette.background) >= 4.5, "\(family) \(variant): the default text")
			for (index, color) in palette.ansi.enumerated() where !backgroundRoles.contains(Self.names[index]) {
				#expect(contrast(color, palette.background) >= 4.5, "\(family) \(variant): \(Self.names[index]) \(color)")
			}
		}
	}
}
