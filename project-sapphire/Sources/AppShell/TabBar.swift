import CoreBindings
import Store
import SwiftUI

/// The tab bar, rendered from the core's tabs; selecting, closing and opening tabs are commands to the store.
struct TabBar: View {
	let store: Store

	var body: some View {
		let window = store.app.window
		HStack(spacing: 2) {
			ForEach(window.tabs, id: \.id) { tab in
				TabButton(
					tab: tab, isActive: tab.id == window.activeTab,
					select: { store.dispatch(.activateTab(id: tab.id)) },
					close: { store.dispatch(.closeTab(id: tab.id)) })
			}
			Button {
				store.dispatch(.openShell)
			} label: {
				Image(systemName: "plus").frame(width: 22, height: 22)
			}
			.buttonStyle(.borderless)
			.help("New Shell")
			Spacer(minLength: 0)
		}
		.padding(.horizontal, 6)
		.padding(.vertical, 4)
		.background(.bar)
	}
}

private struct TabButton: View {
	let tab: AppTab
	let isActive: Bool
	let select: () -> Void
	let close: () -> Void

	var body: some View {
		HStack(spacing: 6) {
			Image(systemName: icon).imageScale(.small).foregroundStyle(.secondary)
			Text(tab.title).lineLimit(1)
			Button(action: close) {
				Image(systemName: "xmark").imageScale(.small)
			}
			.buttonStyle(.borderless)
			.help("Close Tab")
		}
		.padding(.horizontal, 10)
		.padding(.vertical, 4)
		.frame(maxWidth: 220)
		.background(isActive ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 6))
		.contentShape(.rect)
		.onTapGesture(perform: select)
	}

	private var icon: String {
		switch tab.session.kind {
		case .shell: "terminal"
		case .workspace: "folder"
		case .creatingWorkspace, .creationFailed: "hourglass"
		}
	}
}
