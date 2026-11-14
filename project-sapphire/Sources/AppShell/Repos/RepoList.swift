import CoreBindings
import SwiftUI

extension [RepoInfo] {
	/// The repos whose name contains `filter`, trimmed, ignoring case, as Ember's `RepoList` filters them.
	func matching(_ filter: String) -> [RepoInfo] {
		let needle = filter.trimmingCharacters(in: .whitespaces)
		guard !needle.isEmpty else { return self }
		return self.filter { $0.name.localizedCaseInsensitiveContains(needle) }
	}
}

/// The repo picker of the New Agent, New Task and Add Repo dialogs, as a native list: the active row is the list's
/// selection, and in `.multi` each repo is a checkbox (any number chosen), while in `.single` the selection is the
/// choice. The dialog owns the filter field, the cursor and what is chosen.
struct RepoListView: View {
	enum Mode {
		case multi
		case single
	}

	let repos: [RepoInfo]
	/// What the filter field holds, which the empty list names.
	let filter: String
	let active: RepoInfo?
	let mode: Mode
	let isSelected: (RepoInfo) -> Bool
	/// A click on a checkbox or its repo's name (`.multi`).
	let click: (RepoInfo) -> Void
	/// Selecting a row in the list, by a click or the list's own arrow keys.
	let activate: (RepoInfo) -> Void
	/// Each row's accessibility identifier is this followed by the repo's name.
	let identifierPrefix: String
	let height: CGFloat

	var body: some View {
		ScrollViewReader { proxy in
			List(repos, id: \.path, selection: selection) { repo in
				row(repo)
					.id(repo.path)
			}
			.listStyle(.bordered)
			.overlay {
				if repos.isEmpty {
					ContentUnavailableView.search(text: filter.trimmingCharacters(in: .whitespaces))
				}
			}
			.frame(height: height)
			.onChange(of: active?.path) { _, path in
				if let path { proxy.scrollTo(path) }
			}
		}
	}

	private var selection: Binding<String?> {
		Binding(
			get: { active?.path },
			set: { path in
				if let repo = repos.first(where: { $0.path == path }) { activate(repo) }
			})
	}

	@ViewBuilder private func row(_ repo: RepoInfo) -> some View {
		let selected = isSelected(repo)
		Group {
			switch mode {
			case .multi:
				Toggle(isOn: Binding(get: { selected }, set: { _ in click(repo) })) { name(repo) }
					.toggleStyle(.checkbox)
			case .single:
				name(repo)
			}
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.listRowSeparator(.hidden)
		.help(repo.name)
		.accessibilityIdentifier("\(identifierPrefix)\(repo.name)")
	}

	private func name(_ repo: RepoInfo) -> some View {
		Text(repo.name).lineLimit(1).truncationMode(.middle)
	}
}
