@testable import AppShell
import Layers
import Testing

@MainActor
struct CommandPaletteTests {
	@Test func filtersAndNavigatesTheSameCommandsItExecutes() {
		let model = CommandPaletteModel(commands: PaletteCommand.commands(profiles: [], active: nil))
		var executed: PaletteCommand.Action?
		model.run = { executed = $0 }
		model.fieldFocused = true
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.downArrow)))
		#expect(model.selected?.title == "New Shell")
		#expect(!model.handleKey(KeyPress(keyCode: 0x0c, characters: "q")))
		model.filter = "fresh SHELL"
		#expect(model.items.map(\.title) == ["New Shell"])
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.returnKey)))
		#expect(executed == .newShell)
	}

	@Test func escapeClosesEvenWhileFieldIsFocused() {
		let model = CommandPaletteModel(commands: [])
		var closed = false
		model.close = { closed = true }
		model.fieldFocused = true
		#expect(model.handleKey(KeyPress(keyCode: KeyCode.escape)))
		#expect(closed)
	}

	@Test func repurposeDialogCyclesItsControlsAndSubmitsTheTitle() {
		let dialog = RepurposeDialogModel(title: "Next task")
		var submitted: String?
		dialog.submit = { submitted = $0 }
		dialog.cycleFocus(backward: false)
		#expect(dialog.focus == .cancel)
		dialog.cycleFocus(backward: false)
		#expect(dialog.focus == .confirm)
		#expect(dialog.handleKey(KeyPress(keyCode: KeyCode.space)))
		#expect(submitted == "Next task")
	}
}
