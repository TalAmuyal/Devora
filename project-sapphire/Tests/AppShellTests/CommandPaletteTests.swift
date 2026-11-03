@testable import AppShell
import Layers
import CoreBindings
import Foundation
import Store
import Testing

@MainActor
struct CommandPaletteTests {
	@Test func openingDoesNotWaitForProfilesOrWorkspaceLoading() throws {
		let log = try AppLog.open(path: FileManager.default.temporaryDirectory.appending(path: "palette-\(UUID()).log").path)
		let store = Store(configPath: "/nonexistent/config.json", state: .inMemory, log: log)
		let surfaces = Surfaces(store: store)
		surfaces.openCommandPalette()
		#expect(surfaces.palette != nil)
		#expect(surfaces.palette?.fieldFocused == true)
		#expect(surfaces.windowStack.find(CommandPaletteModel.layerName) != nil)
	}

	@Test func refreshingProfilesPreservesFilterAndSelectedCommand() {
		let profiles = [Profile(name: "Work", path: "/work", repoCount: 0), Profile(name: "Other", path: "/other", repoCount: 0)]
		let model = CommandPaletteModel(commands: PaletteCommand.commands(profiles: profiles, active: "/work"))
		model.filter = "profile"
		model.select(2)
		let selected = model.selected?.id
		model.updateProfiles(profiles + [Profile(name: "Third", path: "/third", repoCount: 0)], active: "/work")
		#expect(model.filter == "profile")
		#expect(model.selected?.id == selected)
		#expect(model.items.contains { $0.title == "Switch Profile: Third" })
	}
	@Test func filtersAndNavigatesTheSameCommandsItExecutes() {
		let model = CommandPaletteModel(commands: PaletteCommand.commands(profiles: [], active: nil))
		var executed: PaletteCommand.Action?
		model.run = { executed = $0 }
		model.fieldFocused = true
		#expect(!model.handleKey(KeyPress(keyCode: 0x26, characters: "j")))
		#expect(model.selected?.title == "Workspace Hub")
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

	@Test func typingBeforeTheFieldEditorMountsIsKeptInTheFilter() {
		let model = CommandPaletteModel(commands: [])
		model.fieldFocused = true
		model.isEditingText = { false }
		#expect(model.handleKey(KeyPress(keyCode: 0x0c, characters: "q")))
		#expect(model.filter == "q")
		model.isEditingText = { true }
		#expect(!model.handleKey(KeyPress(keyCode: 0x00, characters: "a")))
		#expect(model.filter == "q")
	}

	@Test func repurposeDialogCyclesItsControlsAndSubmitsTheTitle() {
		let dialog = RepurposeDialogModel(title: "Next task")
		var submitted: String?
		var cancelled = false
		dialog.submit = { submitted = $0 }
		dialog.close = { cancelled = true }
		dialog.cycleFocus(backward: false)
		#expect(dialog.focus == .cancel)
		#expect(dialog.handleKey(KeyPress(keyCode: KeyCode.returnKey)))
		#expect(cancelled)
		#expect(submitted == nil)
		dialog.confirm() // Clicking Repurpose activates that button, even when Cancel held keyboard focus.
		#expect(submitted == "Next task")
		submitted = nil
		dialog.cycleFocus(backward: false)
		#expect(dialog.focus == .confirm)
		#expect(dialog.handleKey(KeyPress(keyCode: KeyCode.space)))
		#expect(submitted == "Next task")
	}
}
