import Layers
import SwiftUI

@MainActor
@Observable
final class RepurposeDialogModel {
	static let layerName = "repurpose-dialog"
	enum Focus: CaseIterable { case title, cancel, confirm }
	var title: String
	var focus = Focus.title {
		didSet { if focus != .title { clearFocus() } }
	}
	@ObservationIgnored var submit: (String) -> Void = { _ in }
	@ObservationIgnored var close: () -> Void = {}
	@ObservationIgnored var clearFocus: () -> Void = {}

	init(title: String) { self.title = title }
	func cycleFocus(backward: Bool) {
		let items = Focus.allCases
		let index = items.firstIndex(of: focus) ?? 0
		focus = items[(index + (backward ? items.count - 1 : 1)) % items.count]
	}
	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.escape: close()
		case KeyCode.returnKey, KeyCode.keypadEnter:
			let value = title.trimmingCharacters(in: .whitespacesAndNewlines)
			if !value.isEmpty { close(); submit(value) }
		case KeyCode.space where focus != .title:
			if focus == .cancel { close() } else { _ = handleKey(KeyPress(keyCode: KeyCode.returnKey)) }
		default: return focus != .title && key.isUnusedOnAPage
		}
		return true
	}
}

struct RepurposeDialogView: View {
	@Bindable var model: RepurposeDialogModel
	@FocusState private var focused: Bool

	var body: some View {
		RepoDialogFrame(identifier: RepurposeDialogModel.layerName, onBackdropClick: model.close) {
			Text("Repurpose Workspace").font(.title3.weight(.semibold))
			TextField("Task title", text: $model.title).textFieldStyle(.roundedBorder)
				.focused($focused).accessibilityIdentifier("repurpose-title")
			HStack {
				Spacer()
				DialogButton(title: "Cancel", isFocused: model.focus == .cancel, action: model.close)
				DialogButton(title: "Repurpose", isFocused: model.focus == .confirm, tint: .accentColor) {
					_ = model.handleKey(Layers.KeyPress(keyCode: KeyCode.returnKey))
				}.disabled(model.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
			}
		}
		.onChange(of: model.focus, initial: true) { _, value in focused = value == .title }
		.onChange(of: focused) { _, value in if value { model.focus = .title } }
	}
}
