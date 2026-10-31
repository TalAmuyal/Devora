import Layers
import SwiftUI

/// What a confirmation dialog asks, as Ember's `showConfirmationDialog` options.
struct Confirmation: Equatable {
	struct Item: Equatable {
		var name: String
		var detail: String
	}

	var title: String
	var intro: String
	/// Listed between the intro and the warning, each with its name in bold.
	var items: [Item] = []
	var warning: String = ""
	var confirmLabel: String
	var cancelLabel = "Cancel"
}

/// One open confirmation dialog: a modal layer over the window (`Surfaces.confirm`).
///
/// Its keys are Ember's: Escape and `q` cancel, Enter confirms whatever has the focus, Tab and Shift+Tab move the focus between its two buttons (the Tab trap), and Space presses the focused one.
/// It draws its own focus, rather than using the window's first responder, so that the trap holds whatever the system's keyboard navigation setting is.
@MainActor
@Observable
final class ConfirmationDialogModel {
	static let layerName = "confirmation-dialog"

	enum Button {
		case cancel
		case confirm
	}

	let confirmation: Confirmation
	/// The confirm button has the focus first, as in Ember.
	private(set) var focused = Button.confirm

	@ObservationIgnored private let onConfirm: () -> Void
	@ObservationIgnored private let onCancel: () -> Void

	/// `onConfirm` and `onCancel` close the dialog; at most one of them runs.
	init(_ confirmation: Confirmation, onConfirm: @escaping () -> Void, onCancel: @escaping () -> Void) {
		self.confirmation = confirmation
		self.onConfirm = onConfirm
		self.onCancel = onCancel
	}

	func confirm() { onConfirm() }
	func cancel() { onCancel() }

	/// Two buttons, so either direction moves to the other one.
	func cycleFocus(backward _: Bool) {
		focused = focused == .confirm ? .cancel : .confirm
	}

	/// Escape cancels and Enter confirms, whatever has the focus; Space presses the focused button; `q` goes on to the stack's dismissal, which cancels; every other plain key is consumed, since nothing beneath may see it.
	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.escape: cancel()
		case KeyCode.returnKey, KeyCode.keypadEnter: confirm()
		case KeyCode.space: focused == .confirm ? confirm() : cancel()
		default: return key.isUnusedOnAPage
		}
		return true
	}
}

/// The dialog over a dimmed backdrop, which cancels it when clicked.
struct ConfirmationDialogView: View {
	let model: ConfirmationDialogModel

	var body: some View {
		let confirmation = model.confirmation
		ZStack {
			Color.black.opacity(0.35)
				.contentShape(.rect)
				.onTapGesture { model.cancel() }
				.accessibilityHidden(true)
			VStack(alignment: .leading, spacing: 12) {
				Text(confirmation.title).font(.title3.weight(.semibold))
				Text(confirmation.intro)
				if !confirmation.items.isEmpty {
					VStack(alignment: .leading, spacing: 4) {
						ForEach(confirmation.items, id: \.name) { item in
							Text("• **\(item.name)** — \(item.detail)")
						}
					}
					.padding(.leading, 8)
				}
				if !confirmation.warning.isEmpty {
					Text(confirmation.warning)
				}
				HStack(spacing: 10) {
					Spacer()
					DialogButton(title: confirmation.cancelLabel, isFocused: model.focused == .cancel, action: model.cancel)
					DialogButton(title: confirmation.confirmLabel, isFocused: model.focused == .confirm, isProminent: true, action: model.confirm)
				}
				.padding(.top, 4)
			}
			.padding(20)
			.frame(width: 460)
			.background(.windowBackground, in: .rect(cornerRadius: 12))
			.shadow(radius: 24)
			.accessibilityElement(children: .contain)
			.accessibilityIdentifier(ConfirmationDialogModel.layerName)
		}
	}
}

private struct DialogButton: View {
	let title: String
	let isFocused: Bool
	var isProminent = false
	let action: () -> Void

	var body: some View {
		Button(action: action) {
			Text(title).frame(minWidth: 70)
		}
		.buttonStyle(.bordered)
		.tint(isProminent ? .red : nil)
		.focusEffectDisabled()
		.overlay {
			if isFocused {
				RoundedRectangle(cornerRadius: 8).stroke(Color.accentColor, lineWidth: 2.5).padding(-3)
			}
		}
		.accessibilityValue(isFocused ? "focused" : "")
	}
}
