import AppKit
import GhosttyKit

/// Decides whether a key event is one of the host's own chords; returning true consumes it before the terminal sees it.
public typealias HostKeyRouter = @MainActor (NSEvent) -> Bool

/// The NSView libghostty renders into (it installs its own Metal layer) and the source of its input.
///
/// The keyboard and IME path is adapted from Ghostty's macOS app (`SurfaceView_AppKit.swift`, MIT license), trimmed to what Sapphire uses.
@MainActor
final class TerminalView: NSView, @preconcurrency NSTextInputClient {
	let session: TerminalSession
	var router: HostKeyRouter?
	var onFocus: (() -> Void)?
	/// Whether the host wants this terminal focused; applied once the view is in a window.
	var wantsFocus = false {
		didSet { claimFocusIfWanted() }
	}
	private(set) var surface: ghostty_surface_t?
	private var markedText = ""
	/// Non-nil while inside keyDown: text the input method commits is collected here and sent with the key.
	private var keyTextAccumulator: [String]?
	private var trackingArea: NSTrackingArea?

	init(session: TerminalSession) {
		self.session = session
		super.init(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
		session.view = self
	}

	@available(*, unavailable)
	required init?(coder: NSCoder) { fatalError() }

	isolated deinit {
		if let surface { ghostty_surface_free(surface) }
	}

	override var acceptsFirstResponder: Bool { true }

	override func viewDidMoveToWindow() {
		super.viewDidMoveToWindow()
		guard surface == nil, let window else { return }
		var config = ghostty_surface_config_new()
		config.platform_tag = GHOSTTY_PLATFORM_MACOS
		let me = Unmanaged.passUnretained(self).toOpaque()
		config.platform = ghostty_platform_u(macos: ghostty_platform_macos_s(nsview: me))
		config.userdata = me
		config.scale_factor = window.backingScaleFactor
		config.context = GHOSTTY_SURFACE_CONTEXT_SPLIT
		let app = TerminalRuntime.shared.app
		surface = withOptionalCString(session.command) { command in
			withOptionalCString(session.workingDirectory) { cwd in
				config.command = command
				config.working_directory = cwd
				return ghostty_surface_new(app, &config)
			}
		}
		session.startedAt = .now
		updateSurfaceSize()
		updateTrackingAreas()
		claimFocusIfWanted()
	}

	/// SwiftUI's first `updateNSView` runs before the view has a window, so focus is applied here too.
	private func claimFocusIfWanted() {
		guard wantsFocus, let window, window.firstResponder !== self else { return }
		window.makeFirstResponder(self)
	}

	override func setFrameSize(_ newSize: NSSize) {
		super.setFrameSize(newSize)
		updateSurfaceSize()
	}

	override func viewDidChangeBackingProperties() {
		super.viewDidChangeBackingProperties()
		guard let surface, let window else { return }
		ghostty_surface_set_content_scale(surface, window.backingScaleFactor, window.backingScaleFactor)
		updateSurfaceSize()
	}

	private func updateSurfaceSize() {
		guard let surface else { return }
		let backing = convertToBacking(bounds.size)
		ghostty_surface_set_size(surface, UInt32(max(1, backing.width)), UInt32(max(1, backing.height)))
	}

	override func becomeFirstResponder() -> Bool {
		if let surface { ghostty_surface_set_focus(surface, true) }
		onFocus?()
		return true
	}

	override func resignFirstResponder() -> Bool {
		if let surface { ghostty_surface_set_focus(surface, false) }
		return true
	}

	// MARK: Keyboard

	override func performKeyEquivalent(with event: NSEvent) -> Bool {
		guard event.type == .keyDown, window?.firstResponder === self else { return false }
		// Host chords first (ADR-003: shortcuts are routed before surfaces), then the app menu.
		if router?(event) == true { return true }
		if event.modifierFlags.contains(.command), event.charactersIgnoringModifiers == "v" {
			session.paste(NSPasteboard.general.string(forType: .string) ?? "")
			return true
		}
		return false
	}

	override func keyDown(with event: NSEvent) {
		if router?(event) == true { return }
		guard surface != nil else {
			interpretKeyEvents([event])
			return
		}
		let action = event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS
		keyTextAccumulator = []
		defer { keyTextAccumulator = nil }
		let hadMarkedText = !markedText.isEmpty
		interpretKeyEvents([event])
		syncPreedit()
		let composing = !markedText.isEmpty || hadMarkedText

		if let committed = keyTextAccumulator, !committed.isEmpty {
			for text in committed { keyAction(action, event: event, text: text) }
		} else if !composing {
			keyAction(action, event: event, text: Self.ghosttyCharacters(event))
		}
	}

	override func keyUp(with event: NSEvent) {
		keyAction(GHOSTTY_ACTION_RELEASE, event: event, text: nil)
	}

	private func keyAction(_ action: ghostty_input_action_e, event: NSEvent, text: String?) {
		guard let surface else { return }
		var key = ghostty_input_key_s()
		key.action = action
		key.keycode = UInt32(event.keyCode)
		key.mods = Self.mods(event.modifierFlags)
		// Control and command never contribute to the text a key produces; everything else may have (Ghostty's heuristic).
		key.consumed_mods = Self.mods(event.modifierFlags.subtracting([.control, .command]))
		if let scalar = event.characters(byApplyingModifiers: [])?.unicodeScalars.first {
			key.unshifted_codepoint = scalar.value
		}
		if let text, !text.isEmpty {
			text.withCString { ptr in
				key.text = ptr
				_ = ghostty_surface_key(surface, key)
			}
		} else {
			_ = ghostty_surface_key(surface, key)
		}
	}

	/// The event's text, minus what Ghostty encodes itself: control characters and the function-key private-use range.
	private static func ghosttyCharacters(_ event: NSEvent) -> String? {
		guard let characters = event.characters else { return nil }
		if characters.count == 1, let scalar = characters.unicodeScalars.first {
			if scalar.value < 0x20 { return event.characters(byApplyingModifiers: event.modifierFlags.subtracting(.control)) }
			if (0xF700...0xF8FF).contains(scalar.value) { return nil }
		}
		return characters
	}

	static func mods(_ flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
		var raw: UInt32 = 0
		if flags.contains(.shift) { raw |= GHOSTTY_MODS_SHIFT.rawValue }
		if flags.contains(.control) { raw |= GHOSTTY_MODS_CTRL.rawValue }
		if flags.contains(.option) { raw |= GHOSTTY_MODS_ALT.rawValue }
		if flags.contains(.command) { raw |= GHOSTTY_MODS_SUPER.rawValue }
		if flags.contains(.capsLock) { raw |= GHOSTTY_MODS_CAPS.rawValue }
		return ghostty_input_mods_e(rawValue: raw)
	}

	private func syncPreedit() {
		guard let surface else { return }
		if markedText.isEmpty {
			ghostty_surface_preedit(surface, nil, 0)
		} else {
			markedText.withCString { ghostty_surface_preedit(surface, $0, UInt(strlen($0))) }
		}
	}

	// MARK: NSTextInputClient

	func insertText(_ string: Any, replacementRange: NSRange) {
		let text = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
		markedText = ""
		if keyTextAccumulator != nil {
			keyTextAccumulator?.append(text)
		} else if let surface {
			text.withCString { ghostty_surface_text(surface, $0, UInt(strlen($0))) }
		}
	}

	func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
		markedText = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
		if keyTextAccumulator == nil { syncPreedit() }
	}

	func unmarkText() {
		markedText = ""
		syncPreedit()
	}

	override func doCommand(by selector: Selector) {}
	func selectedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }
	func markedRange() -> NSRange { markedText.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: markedText.utf16.count) }
	func hasMarkedText() -> Bool { !markedText.isEmpty }
	func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? { nil }
	func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
	func characterIndex(for point: NSPoint) -> Int { NSNotFound }

	func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
		guard let surface, let window else { return .zero }
		var x = 0.0, y = 0.0, width = 0.0, height = 0.0
		ghostty_surface_ime_point(surface, &x, &y, &width, &height)
		// ime_point is in view points with a top-left origin.
		let local = NSRect(x: x, y: bounds.height - y, width: width, height: height)
		return window.convertToScreen(convert(local, to: nil))
	}

	// MARK: Mouse

	override func updateTrackingAreas() {
		if let trackingArea { removeTrackingArea(trackingArea) }
		let area = NSTrackingArea(rect: bounds, options: [.mouseMoved, .inVisibleRect, .activeAlways], owner: self)
		addTrackingArea(area)
		trackingArea = area
		super.updateTrackingAreas()
	}

	override func mouseDown(with event: NSEvent) {
		window?.makeFirstResponder(self)
		mouseButton(event, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT)
	}

	override func mouseUp(with event: NSEvent) { mouseButton(event, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_LEFT) }
	override func rightMouseDown(with event: NSEvent) { mouseButton(event, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT) }
	override func rightMouseUp(with event: NSEvent) { mouseButton(event, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT) }
	override func mouseMoved(with event: NSEvent) { mousePosition(event) }
	override func mouseDragged(with event: NSEvent) { mousePosition(event) }

	private func mouseButton(_ event: NSEvent, _ state: ghostty_input_mouse_state_e, _ button: ghostty_input_mouse_button_e) {
		guard let surface else { return }
		mousePosition(event)
		_ = ghostty_surface_mouse_button(surface, state, button, Self.mods(event.modifierFlags))
	}

	private func mousePosition(_ event: NSEvent) {
		guard let surface else { return }
		let point = convert(event.locationInWindow, from: nil)
		ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, Self.mods(event.modifierFlags))
	}

	override func scrollWheel(with event: NSEvent) {
		guard let surface else { return }
		var dx = event.scrollingDeltaX, dy = event.scrollingDeltaY
		// Precise (trackpad) deltas are in points; Ghostty's own app doubles them for a natural speed.
		if event.hasPreciseScrollingDeltas { dx *= 2; dy *= 2 }
		ghostty_surface_mouse_scroll(surface, dx, dy, event.hasPreciseScrollingDeltas ? 1 : 0)
	}
}

private func withOptionalCString<R>(_ string: String?, _ body: (UnsafePointer<CChar>?) -> R) -> R {
	guard let string else { return body(nil) }
	return string.withCString { body($0) }
}
