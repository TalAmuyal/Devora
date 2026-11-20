import AppKit
@testable import AppShell
import CoreBindings
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct AppAppearanceTests {
	/// Automatic leaves the app's appearance unset, so that it follows the system; Light and Dark fix it.
	@Test func theSettingMapsToTheAppsAppearance() {
		#expect(AppAppearance.appKit(.automatic) == nil)
		#expect(AppAppearance.appKit(.light)?.name == .aqua)
		#expect(AppAppearance.appKit(.dark)?.name == .darkAqua)
	}

	@Test func theEffectiveAppearanceIsLightOrDark() throws {
		#expect(AppAppearance.effective(of: try #require(NSAppearance(named: .aqua))) == .light)
		#expect(AppAppearance.effective(of: try #require(NSAppearance(named: .darkAqua))) == .dark)
		#expect(AppAppearance.effective(of: try #require(NSAppearance(named: .accessibilityHighContrastDarkAqua))) == .dark)
	}
}
