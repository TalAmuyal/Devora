import CoreBindings

extension AppState {
	/// The state before the core's first snapshot arrives.
	public static let empty = AppState(window: AppWindow(tabs: [], activeTab: nil), activeProfile: nil)

	/// Applies one of the core's events, the same change its reducer made.
	public mutating func apply(_ event: StateEvent) {
		switch event {
		case .snapshot(let state):
			self = state
		case .sessionOpened(let tab, let index):
			window.tabs.insert(tab, at: min(Int(index), window.tabs.count))
		case .sessionClosed(let id):
			window.tabs.removeAll { $0.id == id }
		case .tabActivated(let id):
			window.activeTab = id
		case .tabMoved(let id, _, let to):
			guard let from = window.tabs.firstIndex(where: { $0.id == id }) else { return }
			let tab = window.tabs.remove(at: from)
			window.tabs.insert(tab, at: min(Int(to), window.tabs.count))
		case .titleChanged(let id, let title):
			guard let i = window.tabs.firstIndex(where: { $0.id == id }) else { return }
			window.tabs[i].title = title
		case .sessionChanged(let id, let session):
			guard let i = window.tabs.firstIndex(where: { $0.id == id }) else { return }
			window.tabs[i].session = session
		case .activeProfileChanged(let path):
			activeProfile = path
		}
	}
}

extension AppTab {
	/// The id of the terminal in the tab's pane; the UI keeps the live terminal under it.
	public var terminalID: UInt64 {
		switch pane {
		case .terminal(let id): id
		}
	}
}
