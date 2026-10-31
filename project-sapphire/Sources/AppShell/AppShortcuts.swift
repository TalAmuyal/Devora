import Layers
import Store
import TerminalKit

/// An app-wide shortcut: a chord the router runs before any surface sees it (`Sources/Layers/CLAUDE.md` has the availability table).
enum AppShortcut: Equatable {
	case setFontSize(Double)
	case changeFontSize(by: Double)
	case previousTab
	case nextTab
	case moveTabBackward
	case moveTabForward
	case newShell
	case openWorkspaceHub

	/// The shortcut a key-down is, if any; matched on the physical key, since Control turns the characters into control codes.
	/// Command and Option chords are never shortcuts, so they reach the menu and the terminal unchanged.
	init?(_ key: KeyPress) {
		guard key.modifiers.contains(.control), key.modifiers.isDisjoint(with: [.command, .option]) else { return nil }
		let shift = key.modifiers.contains(.shift)
		let reset = TerminalRuntime.defaultFontSize
		switch (key.keyCode, shift) {
		case (KeyCode.leftArrow, false): self = .previousTab
		case (KeyCode.rightArrow, false): self = .nextTab
		case (KeyCode.leftArrow, true): self = .moveTabBackward
		case (KeyCode.rightArrow, true): self = .moveTabForward
		case (KeyCode.s, true): self = .newShell
		case (KeyCode.s, false): self = .openWorkspaceHub
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
		case .setFontSize, .changeFontSize: false
		case .previousTab, .nextTab, .moveTabBackward, .moveTabForward, .newShell, .openWorkspaceHub: true
		}
	}
}

/// The app-wide shortcut policy: the router's first stage.
@MainActor
struct AppShortcuts {
	static let fontSizes: ClosedRange<Double> = 8...40

	let store: Store
	let windowStack: LayerStack
	let openWorkspaceHub: () -> Void

	/// Returns whether the key was a shortcut, which consumes it even when unavailable: a deliberate no-op, so the key reaches neither the surface that made it unavailable nor a terminal.
	func handle(_ key: KeyPress) -> Bool {
		guard let shortcut = AppShortcut(key) else { return false }
		if shortcut.needsNoWindowSurface, windowStack.hasOpaqueLayer { return true }
		run(shortcut)
		return true
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
		case .openWorkspaceHub: openWorkspaceHub()
		}
	}
}

extension Comparable {
	func clamped(to range: ClosedRange<Self>) -> Self {
		min(max(self, range.lowerBound), range.upperBound)
	}
}
