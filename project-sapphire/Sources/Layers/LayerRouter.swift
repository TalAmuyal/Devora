import AppKit

/// The one owner of key routing and of where the keyboard focus goes (ADR-003), for one window.
///
/// Every key-down in the window enters through `handle`, before AppKit dispatches it, and runs three stages in order: the app-wide shortcuts, the window stack top-down, then the active tab's stack.
/// Shortcuts run first, so each one states its own availability and no surface can shadow one by accident.
/// A stack that reports `blocked` ends routing without consuming the key, so it still reaches the first responder: that is how a terminal layer receives typing while staying an opaque barrier.
@MainActor
public final class LayerRouter {
	/// The app-covering stack: hubs, the Command Palette, dialogs.
	public let windowStack: LayerStack
	/// The app-wide shortcut policy, tried before any layer; returning true consumes the key.
	public var shortcuts: ((KeyPress) -> Bool)?

	private let activeTabStack: @MainActor () -> LayerStack?
	private let focus: WindowFocus
	private var monitor: Any?

	/// - Parameters:
	///   - activeTabStack: the active tab's stack, resolved on each use so it can't go stale; nil without tabs.
	///   - fallbackFocus: takes the focus away from whatever has it, for when there is nothing to focus; by default, the window's first responder is cleared, so keys reach no view.
	///   - isEditingText: whether a text field or text view has the focus; by default, whether the window's first responder is one (a field editor included).
	public init(
		activeTabStack: @escaping @MainActor () -> LayerStack?,
		fallbackFocus: (@MainActor () -> Void)? = nil,
		isEditingText: (@MainActor () -> Bool)? = nil
	) {
		self.activeTabStack = activeTabStack
		let focus = WindowFocus(fallback: fallbackFocus, isEditingText: isEditingText)
		self.focus = focus
		windowStack = LayerStack(fallbackFocus: focus.clear, isEditingText: focus.isEditingText)
		windowStack.onEmptied = { [unowned self] in focusActiveTab() }
	}

	/// Makes a stack for a tab: its top takes the focus only while `isActive` holds and no window surface is open.
	public func makeTabStack(isActive: @escaping @MainActor () -> Bool) -> LayerStack {
		LayerStack(
			isFocusOwner: { [unowned self] in isActive() && windowStack.isEmpty },
			fallbackFocus: focus.clear,
			isEditingText: focus.isEditingText)
	}

	/// Routes one key-down; returns true when it is consumed, so AppKit must not dispatch it.
	public func handle(_ key: KeyPress) -> Bool {
		if shortcuts?(key) == true { return true }
		switch windowStack.route(key) {
		case .consumed: return true
		case .blocked: return false
		case .passed: return activeTabStack()?.route(key) == .consumed
		}
	}

	/// Gives the focus to the active tab's top layer, unless a window surface has it; with nothing to focus, takes it from whatever had it.
	/// Called when the active tab changes, and when the window stack empties.
	public func focusActiveTab() {
		guard windowStack.isEmpty else { return }
		if activeTabStack()?.focusTop() != true { focus.clear() }
	}

	/// Routes every key-down meant for `window` through `handle` before AppKit dispatches it, so neither the menu, the first responder nor libghostty sees a consumed key.
	public func install(in window: NSWindow) {
		uninstall()
		focus.window = window
		monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self, weak window] event in
			let key = KeyPress(event)
			let windowNumber = event.windowNumber
			let consumed = MainActor.assumeIsolated {
				guard let self, let window, window.windowNumber == windowNumber else { return false }
				return self.handle(key)
			}
			return consumed ? nil : event
		}
	}

	public func uninstall() {
		if let monitor { NSEvent.removeMonitor(monitor) }
		monitor = nil
	}
}

/// The router's view of the window's focus: taking it away when there is nothing to focus, and whether text is being edited; both are injectable for tests.
@MainActor
private final class WindowFocus {
	let injectedClear: (@MainActor () -> Void)?
	let injectedIsEditingText: (@MainActor () -> Bool)?
	weak var window: NSWindow?

	init(fallback: (@MainActor () -> Void)?, isEditingText: (@MainActor () -> Bool)?) {
		injectedClear = fallback
		injectedIsEditingText = isEditingText
	}

	/// Clears the window's first responder, so keys reach no view.
	func clear() {
		if let injectedClear { injectedClear() } else { window?.makeFirstResponder(nil) }
	}

	/// An `NSTextField` edits through its window's field editor, an `NSTextView`, so this covers both.
	func isEditingText() -> Bool {
		injectedIsEditingText?() ?? (window?.firstResponder is NSTextView)
	}
}
