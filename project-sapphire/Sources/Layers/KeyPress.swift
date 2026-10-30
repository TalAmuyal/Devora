import AppKit

/// A key-down, as the router and the layers see it: the physical key, the modifiers held, and the text it produces.
public struct KeyPress: Sendable, Equatable {
	/// The physical key (a virtual key code, `kVK_*`), independent of the keyboard layout and of the modifiers.
	public var keyCode: UInt16
	public var modifiers: KeyModifiers
	/// The text the key produces with its modifiers; with Control held this is a control character, so chords are matched on `keyCode`.
	public var characters: String
	public var isRepeat: Bool

	public init(keyCode: UInt16, modifiers: KeyModifiers = [], characters: String = "", isRepeat: Bool = false) {
		self.keyCode = keyCode
		self.modifiers = modifiers
		self.characters = characters
		self.isRepeat = isRepeat
	}

	public init(_ event: NSEvent) {
		self.init(
			keyCode: event.keyCode, modifiers: KeyModifiers(event.modifierFlags),
			characters: event.characters ?? "", isRepeat: event.isARepeat)
	}
}

/// The modifiers a chord is made of; AppKit's other flags (Caps Lock, Fn, the numeric pad) never change which chord a key is.
public struct KeyModifiers: OptionSet, Sendable, Hashable {
	public let rawValue: UInt8
	public init(rawValue: UInt8) { self.rawValue = rawValue }

	public static let shift = KeyModifiers(rawValue: 1 << 0)
	public static let control = KeyModifiers(rawValue: 1 << 1)
	public static let option = KeyModifiers(rawValue: 1 << 2)
	public static let command = KeyModifiers(rawValue: 1 << 3)

	public init(_ flags: NSEvent.ModifierFlags) {
		self = []
		if flags.contains(.shift) { insert(.shift) }
		if flags.contains(.control) { insert(.control) }
		if flags.contains(.option) { insert(.option) }
		if flags.contains(.command) { insert(.command) }
	}
}

/// The physical keys Sapphire's own chords and the layers use (`kVK_*` from Carbon's `Events.h`).
public enum KeyCode {
	public static let s: UInt16 = 0x01
	public static let digit1: UInt16 = 0x12
	public static let digit2: UInt16 = 0x13
	public static let digit3: UInt16 = 0x14
	public static let equal: UInt16 = 0x18
	public static let minus: UInt16 = 0x1B
	public static let escape: UInt16 = 0x35
	public static let keypadPlus: UInt16 = 0x45
	public static let keypadMinus: UInt16 = 0x4E
	public static let leftArrow: UInt16 = 0x7B
	public static let rightArrow: UInt16 = 0x7C
}
