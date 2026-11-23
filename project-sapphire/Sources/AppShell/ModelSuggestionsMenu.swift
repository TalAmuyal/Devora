import AppKit
import Store
import SwiftUI

extension View {
	/// The models Claude Code offers, as a text field's native suggestions menu, in `rows`' order. The menu is as wide as
	/// its field, so a narrow field's rows show only their labels; a wide one's (`showsModelIDs`) show the label
	/// on the left and the full ID on the right, in a monospaced column as wide as the
	/// longest ID, so that every ID starts at the same place. With no suggestions the field is a plain text field.
	func modelSuggestions(_ rows: [ModelSuggestions.Row], showsModelIDs: Bool) -> some View {
		let width = ModelSuggestionsMenu.idColumnWidth(rows)
		return textInputSuggestions {
			// A section per kind, "Aliases" then "Model IDs", with a separator line between them; a tier's field has
			// only the second, still labelled, so the fields look alike.
			ForEach(ModelSuggestionsMenu.sections(rows), id: \.0) { section, sectionRows in
				// A section's label alone doesn't draw a line, so the line between the sections is drawn here.
				if section == .modelIDs, rows.contains(where: { $0.section == .aliases }) { Divider() }
				Section(section.rawValue) {
					ForEach(sectionRows) { row in
						Group {
							if showsModelIDs {
								HStack(spacing: 16) {
									Text(row.name).lineLimit(1)
										.frame(maxWidth: .infinity, alignment: .leading)
									Text(row.modelID).monospaced().foregroundStyle(.secondary)
										.frame(width: width, alignment: .leading)
										.layoutPriority(1)
								}
							} else {
								Text(row.name).lineLimit(1)
							}
						}
						.textInputCompletion(row.completion)
					}
				}
			}
		}
	}
}

enum ModelSuggestionsMenu {
	/// The rows by section, in the sections' order, leaving out a section with none.
	static func sections(_ rows: [ModelSuggestions.Row]) -> [(ModelSuggestions.Row.Section, [ModelSuggestions.Row])] {
		[ModelSuggestions.Row.Section.aliases, .modelIDs].compactMap { section in
			let inSection = rows.filter { $0.section == section }
			return inSection.isEmpty ? nil : (section, inSection)
		}
	}

	static func idColumnWidth(_ rows: [ModelSuggestions.Row]) -> CGFloat {
		let font = NSFont.monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
		return rows.map { ceil(($0.modelID as NSString).size(withAttributes: [.font: font]).width) }.max() ?? 0
	}
}
