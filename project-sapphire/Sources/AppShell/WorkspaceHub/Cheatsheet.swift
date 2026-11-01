import SwiftUI

/// The Workspace Hub's shortcuts, a page of its own over the hub (`?`), scrolled with Vim's keys.
struct Cheatsheet: View {
	static let layerName = "cheatsheet"

	@Bindable var scroll: VimScroll

	private static let sections: [(heading: String, keys: [(String, String)])] = [
		("Workspace Hub", [
			("j / ↓", "Move selection down"),
			("k / ↑", "Move selection up"),
			("Enter", "Open selected workspace"),
			("f", "Focus filter input"),
			("Esc", "Unfocus filter / close cheatsheet"),
			("1", "Show active workspaces"),
			("2", "Show inactive workspaces"),
			("3", "Show all workspaces"),
			("n", "New task"),
			("P", "Manage Profiles"),
			("R", "Refresh hub"),
			("q", "Close hub"),
			("?", "Toggle this cheatsheet"),
		]),
		("Global", [
			("Ctrl+S", "Open Workspace Hub"),
			("Ctrl+Shift+S", "New shell tab"),
			("Ctrl+←/→", "Switch tabs"),
			("Ctrl+Shift+←/→", "Reorder tabs"),
			("Ctrl+Shift++", "Increase terminal font size"),
			("Ctrl+Shift+-", "Decrease terminal font size"),
			("Ctrl+=", "Reset terminal font size"),
			("Ctrl+1/2/3", "Set terminal font size (small/medium/large)"),
			("Esc", "Dismiss overlay"),
		]),
	]

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: 18) {
				Text("Keyboard Shortcuts").font(.largeTitle.weight(.semibold))
				ForEach(Self.sections, id: \.heading) { section in
					VStack(alignment: .leading, spacing: 10) {
						Text(section.heading).font(.title3.weight(.semibold))
						Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 12) {
							ForEach(section.keys, id: \.1) { keys, description in
								GridRow {
									KeyCap(keys).gridColumnAlignment(.trailing)
									Text(description)
								}
							}
						}
					}
				}
				Text("Scroll: j/k · Ctrl+D/U · gg/G      Press ? or Esc or q to go back")
					.font(.callout)
					.foregroundStyle(.secondary)
			}
			.font(.title3)
			.padding(32)
			.frame(maxWidth: .infinity, alignment: .leading)
		}
		.scrollPosition($scroll.position)
		.onScrollGeometryChange(for: VimScroll.Geometry.self) { geometry in
			VimScroll.Geometry(
				offset: geometry.contentOffset.y + geometry.contentInsets.top,
				contentHeight: geometry.contentSize.height,
				viewportHeight: geometry.containerSize.height)
		} action: { _, geometry in
			scroll.geometry = geometry
		}
		.background(.windowBackground)
		.accessibilityIdentifier("hub-cheatsheet")
	}
}
