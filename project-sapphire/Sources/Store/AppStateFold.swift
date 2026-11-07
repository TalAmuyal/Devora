import CoreBindings

extension AppState {
	/// The state before the core's first snapshot arrives.
	public static let empty = AppState(globalPaused: false, window: AppWindow(tabs: [], activeTab: nil), activeProfile: nil, agentViews: [])

	/// Applies one of the core's events, the same change its reducer made.
	public mutating func apply(_ event: StateEvent) {
		switch event {
		case .globalPauseChanged(let paused): globalPaused = paused
		case .agentDraftCleared(let profile, let name, let text):
			if let index = agentViews.firstIndex(where: { $0.profilePath == profile && $0.name == name && $0.draft == text }) { agentViews[index].draft = "" }
		case .agentViewChanged(let profile, let name, let view):
			agentViews.removeAll { $0.profilePath == profile && $0.name == name }
			if let view { agentViews.append(view) }
			agentViews.sort { ($0.profilePath, $0.name) < ($1.profilePath, $1.name) }
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
		case .panesChanged(let id, let pane):
			guard let i = window.tabs.firstIndex(where: { $0.id == id }) else { return }
			window.tabs[i].pane = pane
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
		case .previewRow(let terminal, _): terminal
		}
	}
}

extension AppTab {
	public var previews: [PreviewPane] {
		switch pane {
		case .terminal: []
		case .previewRow(_, let previews): previews
		}
	}
}
