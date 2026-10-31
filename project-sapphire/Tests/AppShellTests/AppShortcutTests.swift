@testable import AppShell
import Layers
import Testing

/// Which key-downs are app-wide shortcuts: the keys are Ember's, matched on the physical key.
struct AppShortcutTests {
	@Test func controlChordsMatchOnThePhysicalKeyWhateverTheCharacters() {
		// With Shift, the minus key types "_", and Control turns letters into control codes.
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.minus, modifiers: [.control, .shift], characters: "_")) == .changeFontSize(by: -2))
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.equal, modifiers: [.control, .shift], characters: "+")) == .changeFontSize(by: 2))
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.s, modifiers: [.control, .shift], characters: "\u{13}")) == .newShell)
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.s, modifiers: .control, characters: "\u{13}")) == .openWorkspaceHub)
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.digit3, modifiers: .control, characters: "3")) == .setFontSize(26))
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.equal, modifiers: .control, characters: "=")) == .setFontSize(15))
	}

	@Test func optionAndCommandChordsAreLeftForTheMenuAndTheTerminal() {
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.rightArrow, modifiers: [.control, .option])) == nil)
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.rightArrow, modifiers: [.control, .command])) == nil)
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.rightArrow, modifiers: .option)) == nil)
	}

	@Test func controlKeysWithoutAShortcutReachTheTerminal() {
		// Ctrl+C interrupts; Ctrl+- is the shell's.
		#expect(AppShortcut(KeyPress(keyCode: 0x08, modifiers: .control, characters: "\u{03}")) == nil)
		#expect(AppShortcut(KeyPress(keyCode: KeyCode.minus, modifiers: .control, characters: "-")) == nil)
	}
}
