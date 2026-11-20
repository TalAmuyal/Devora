import AppKit
import CoreBindings
import Observation

/// The app's appearance (ADR-010): it applies the User Defaults' Appearance to the app, so that every native view follows it, and publishes the effective one, light or dark, which the panes Sapphire draws itself read.
/// On Automatic the effective appearance is the system's, and follows it as it changes.
@MainActor
@Observable
public final class AppAppearance {
	public enum Effective: String {
		case light
		case dark
	}

	public private(set) var effective: Effective
	@ObservationIgnored private var observation: NSKeyValueObservation?

	init() {
		effective = Self.effective(of: NSApp.effectiveAppearance)
		observation = NSApp.observe(\.effectiveAppearance) { [weak self] app, _ in
			MainActor.assumeIsolated { self?.effective = Self.effective(of: app.effectiveAppearance) }
		}
	}

	/// Applied before the window first shows, so that a user on Dark with a light system sees no light frame, and then live.
	func apply(_ setting: Appearance) {
		NSApp.appearance = Self.appKit(setting)
	}

	/// The app's `NSAppearance` for a setting: none, so that it follows the system, on Automatic.
	static func appKit(_ setting: Appearance) -> NSAppearance? {
		switch setting {
		case .automatic: nil
		case .light: NSAppearance(named: .aqua)
		case .dark: NSAppearance(named: .darkAqua)
		}
	}

	static func effective(of appearance: NSAppearance) -> Effective {
		appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? .dark : .light
	}

	/// The system's appearance, whatever the app's is, for the acceptance tests to compare Automatic with.
	static var system: Effective {
		UserDefaults.standard.string(forKey: "AppleInterfaceStyle") == "Dark" ? .dark : .light
	}
}
