import AppKit
import Layers
import Store
import TerminalKit

/// An app-wide shortcut: a chord the router runs before any surface sees it (`Sources/Layers/CLAUDE.md` has the availability table).
enum AppShortcut: Equatable {
	private struct Chord {
		let keyCode: UInt16
		let modifiers: KeyModifiers
		let keyLabel: String
		var label: String {
			(modifiers.contains(.control) ? "⌃" : "") + (modifiers.contains(.shift) ? "⇧" : "") + keyLabel
		}
		func matches(_ key: KeyPress) -> Bool { key.keyCode == keyCode && key.modifiers == modifiers }
	}
	private static let hubsChord = Chord(keyCode: KeyCode.s, modifiers: .control, keyLabel: "S")
	private static let newShellChord = Chord(keyCode: KeyCode.s, modifiers: [.control, .shift], keyLabel: "S")
	case setFontSize(Double)
	case changeFontSize(by: Double)
	case previousTab
	case nextTab
	case moveTabBackward
	case moveTabForward
	case newShell
	/// Opens the Agent Hub, or switches between the two hubs inside one (ADR-012).
	case hubs
	case openUserGuide
	case openCommandPalette

	/// The shortcut a key-down is, if any; matched on the physical key, since Control turns the characters into control codes.
	/// Command and Option chords are never shortcuts, so they reach the menu and the terminal unchanged.
	init?(_ key: KeyPress) {
		if key.keyCode == KeyCode.f1 { self = .openUserGuide; return }
		if Self.hubsChord.matches(key) { self = .hubs; return }
		if Self.newShellChord.matches(key) { self = .newShell; return }
		guard key.modifiers.contains(.control), key.modifiers.isDisjoint(with: [.command, .option]) else { return nil }
		let shift = key.modifiers.contains(.shift)
		let reset = TerminalRuntime.defaultFontSize
		switch (key.keyCode, shift) {
		case (KeyCode.leftArrow, false): self = .previousTab
		case (KeyCode.rightArrow, false): self = .nextTab
		case (KeyCode.leftArrow, true): self = .moveTabBackward
		case (KeyCode.rightArrow, true): self = .moveTabForward
		case (KeyCode.equal, true), (KeyCode.keypadPlus, true): self = .changeFontSize(by: 2)
		case (KeyCode.minus, true), (KeyCode.keypadMinus, true): self = .changeFontSize(by: -2)
		case (KeyCode.equal, false), (KeyCode.digit2, false): self = .setFontSize(reset)
		case (KeyCode.digit1, false): self = .setFontSize(12)
		case (KeyCode.digit3, false): self = .setFontSize(26)
		default: return nil
		}
	}

	/// Font size is a display preference, which no surface has a reason to make unavailable; the rest open a window surface or switch tabs, so an open window surface makes them unavailable.
	var needsNoWindowSurface: Bool {
		switch self {
		case .setFontSize, .changeFontSize, .openUserGuide: false
		case .previousTab, .nextTab, .moveTabBackward, .moveTabForward, .newShell, .openCommandPalette: true
		// A hub decides itself, since inside one it switches to the other.
		case .hubs: false
		}
	}

	/// The label shown alongside a palette command using this shortcut.
	var paletteLabel: String? {
		switch self {
		case .hubs: Self.hubsChord.label
		case .newShell: Self.newShellChord.label
		default: nil
		}
	}
}

/// The app-wide shortcut policy: the router's first stage.
@MainActor
final class AppShortcuts {
	static let fontSizes: ClosedRange<Double> = 8...40

	let store: Store
	let windowStack: LayerStack
	/// What `Ctrl+S` does, which depends on the window stack (`Surfaces.hubShortcut`).
	let hubs: () -> Void
	let openUserGuide: () -> Void
	var openCommandPalette: () -> Void = {}
	private var shiftTap = ShiftDoubleTap()
	private var flagsMonitor: Any?

	init(store: Store, windowStack: LayerStack, hubs: @escaping () -> Void, openUserGuide: @escaping () -> Void) {
		self.store = store
		self.windowStack = windowStack
		self.hubs = hubs
		self.openUserGuide = openUserGuide
	}

	/// Returns whether the key was a shortcut, which consumes it even when unavailable: a deliberate no-op, so the key reaches neither the surface that made it unavailable nor a terminal.
	func handle(_ key: KeyPress) -> Bool {
		shiftTap.reset()
		guard let shortcut = AppShortcut(key) else { return false }
		if !available(shortcut) { return true }
		run(shortcut)
		return true
	}

	private func available(_ shortcut: AppShortcut) -> Bool {
		!shortcut.needsNoWindowSurface || !windowStack.hasOpaqueLayer
	}

	func handleFlags(keyCode: UInt16, modifiers: KeyModifiers, at time: TimeInterval) {
		let isAvailable = available(.openCommandPalette)
		let fired = shiftTap.flagsChanged(keyCode: keyCode, modifiers: modifiers, at: time, available: isAvailable)
		if fired {
			run(.openCommandPalette)
		}
	}

	/// Modifier events aren't key-downs and must continue to AppKit unchanged.
	func install(in window: NSWindow) {
		uninstall()
		flagsMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self, weak window] event in
			MainActor.assumeIsolated {
				guard let self, let window, event.windowNumber == window.windowNumber else { return }
				self.handleFlags(keyCode: event.keyCode, modifiers: KeyModifiers(event.modifierFlags), at: event.timestamp)
			}
			return event
		}
	}

	func uninstall() {
		if let flagsMonitor { NSEvent.removeMonitor(flagsMonitor) }
		flagsMonitor = nil
		shiftTap.reset()
	}

	private func run(_ shortcut: AppShortcut) {
		let terminals = TerminalRuntime.shared
		switch shortcut {
		case .setFontSize(let points):
			terminals.setFontSize(points)
		case .changeFontSize(let delta):
			terminals.setFontSize((terminals.fontSize + delta).clamped(to: Self.fontSizes))
		case .previousTab: store.dispatch(.activatePreviousTab)
		case .nextTab: store.dispatch(.activateNextTab)
		case .moveTabBackward: store.dispatch(.moveActiveTabBackward)
		case .moveTabForward: store.dispatch(.moveActiveTabForward)
		case .newShell: store.dispatch(.openShell)
		case .hubs: hubs()
		case .openUserGuide: openUserGuide()
		case .openCommandPalette: openCommandPalette()
		}
	}
}

extension Comparable {
	func clamped(to range: ClosedRange<Self>) -> Self {
		min(max(self, range.lowerBound), range.upperBound)
	}
}
