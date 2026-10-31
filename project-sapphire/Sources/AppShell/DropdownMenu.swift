import SwiftUI

/// A row of a dropdown, as Ember's `DropdownItem`: an option (a profile, with a detail and the active one checked), a separator, or an action.
struct DropdownItem {
	enum Kind: String {
		case option
		case separator
		case action
	}

	var kind: Kind
	var label = ""
	var detail: String?
	var checked = false
	var systemImage: String?
	/// A disabled row is drawn dimmed and does nothing.
	var enabled = true
	var help: String?
	var select: () -> Void = {}

	static var separator: DropdownItem { DropdownItem(kind: .separator) }

	static func option(_ label: String, detail: String?, checked: Bool, select: @escaping () -> Void) -> DropdownItem {
		DropdownItem(kind: .option, label: label, detail: detail, checked: checked, select: select)
	}

	static func action(
		_ label: String, systemImage: String, enabled: Bool = true, help: String? = nil, select: @escaping () -> Void = {}
	) -> DropdownItem {
		DropdownItem(kind: .action, label: label, systemImage: systemImage, enabled: enabled, help: help, select: select)
	}
}

/// A dropdown's open panel; the surface it belongs to draws it, anchored under its trigger, while a popup layer routes its keys (`Sources/Layers/CLAUDE.md`).
struct DropdownPanel: View {
	let items: [DropdownItem]
	/// Runs before a row's `select`, which closes the dropdown.
	let close: () -> Void

	var body: some View {
		VStack(alignment: .leading, spacing: 0) {
			ForEach(items.indices, id: \.self) { index in
				let item = items[index]
				if item.kind == .separator {
					Divider().padding(.vertical, 4)
				} else {
					DropdownRow(item: item) {
						close()
						item.select()
					}
				}
			}
		}
		.padding(5)
		.frame(minWidth: 220)
		.fixedSize()
		.background(.regularMaterial, in: .rect(cornerRadius: 8))
		.overlay(RoundedRectangle(cornerRadius: 8).stroke(.separator))
		.shadow(radius: 10, y: 4)
	}
}

private struct DropdownRow: View {
	let item: DropdownItem
	let activate: () -> Void
	@State private var hovered = false

	var body: some View {
		Button(action: activate) {
			HStack(spacing: 8) {
				Group {
					if item.kind == .option {
						Text(item.checked ? "✓" : "")
					} else if let systemImage = item.systemImage {
						Image(systemName: systemImage)
					}
				}
				.frame(width: 16)
				Text(item.label)
				Spacer(minLength: 16)
				if let detail = item.detail {
					Text(detail).foregroundStyle(.secondary)
				}
			}
			.padding(.horizontal, 8)
			.padding(.vertical, 5)
			.frame(maxWidth: .infinity, alignment: .leading)
			.background(hovered && item.enabled ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 5))
			.contentShape(.rect)
		}
		.buttonStyle(.plain)
		.accessibilityLabel(item.label)
		.accessibilityValue(item.detail ?? "")
		.accessibilityAddTraits(item.checked ? .isSelected : [])
		.disabled(!item.enabled)
		.onHover { hovered = $0 }
		.help(item.help ?? "")
	}
}
