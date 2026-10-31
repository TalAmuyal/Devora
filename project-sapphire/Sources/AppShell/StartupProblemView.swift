import AppKit
import Store
import SwiftUI

/// Fills the window when the app state couldn't be opened: quit, or (for an unreadable state) start fresh with the old state kept aside.
///
/// In the window rather than in an alert, since AppKit won't quit while an alert's sheet is up.
struct StartupProblemView: View {
	let problem: StartupProblem
	let store: Store

	var body: some View {
		ContentUnavailableView {
			Label(title, systemImage: "exclamationmark.triangle")
		} description: {
			Text(message).textSelection(.enabled)
		} actions: {
			HStack {
				Button("Quit") { NSApp.terminate(nil) }
					.keyboardShortcut(.cancelAction)
				if case .stateUnreadable = problem {
					Button("Start Fresh") { store.dispatch(.startFresh) }
						.keyboardShortcut(.defaultAction)
				}
			}
		}
	}

	private var title: String {
		switch problem {
		case .stateInUse: "Devora Sapphire is already running"
		case .stateUnreadable: "The saved tabs couldn't be opened"
		}
	}

	private var message: String {
		switch problem {
		case .stateInUse(let path):
			"Another Devora Sapphire is using the saved tabs at \(path), so this one can't open them. Use the one that's running, or quit it first."
		case .stateUnreadable(let path, let reason):
			"\(reason)\n\nStart Fresh opens without the saved tabs, keeping the old state beside \(path) under a timestamped name. Quit leaves it untouched."
		}
	}
}
