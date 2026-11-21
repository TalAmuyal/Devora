import AppKit
import Store
import SwiftUI

extension View {
	/// The models Claude Code offers, as a text field's native suggestions menu: each section under its heading. The menu
	/// is as wide as its field, so a narrow field's rows show only their names; a wide one's (`showsModelIDs`) show the name,
	/// with the stored alias beside it, on the left and the full ID on the right, in a monospaced column as wide as the
	/// longest ID, so that every ID starts at the same place. With no suggestions the field is a plain text field.
	func modelSuggestions(_ sections: [ModelSuggestions.Section], showsModelIDs: Bool) -> some View {
		let width = ModelSuggestionsMenu.idColumnWidth(sections)
		return textInputSuggestions {
			ForEach(sections) { section in
				Section(section.title) {
					ForEach(section.rows) { row in
						Group {
							if showsModelIDs {
								HStack(spacing: 16) {
									(Text(row.name) + Text(row.alias.map { " · \($0)" } ?? "").foregroundStyle(.secondary))
										.lineLimit(1)
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
	static func idColumnWidth(_ sections: [ModelSuggestions.Section]) -> CGFloat {
		let font = NSFont.monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
		return sections.flatMap(\.rows).map { ceil(($0.modelID as NSString).size(withAttributes: [.font: font]).width) }.max() ?? 0
	}
}
