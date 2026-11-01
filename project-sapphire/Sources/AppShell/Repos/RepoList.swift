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

/// The rows of Ember's `RepoList`, shared by the New Task dialog (checkboxes, any number selected) and the Add Repo dialog (radio buttons, the active row being the one selected); the dialog owns the filter field and the selection.
struct RepoListView: View {
	enum Mode {
		case multi
		case single
	}

	let repos: [RepoInfo]
	let active: RepoInfo?
	let mode: Mode
	let isSelected: (RepoInfo) -> Bool
	let click: (RepoInfo) -> Void
	/// Each row's accessibility identifier is this followed by the repo's name.
	let identifierPrefix: String

	var body: some View {
		ScrollViewReader { proxy in
			ScrollView {
				LazyVStack(alignment: .leading, spacing: 2) {
					if repos.isEmpty {
						Text("No matching repos").foregroundStyle(.secondary).padding(8)
					}
					ForEach(repos, id: \.path) { repo in
						let selected = isSelected(repo)
						HStack(spacing: 8) {
							Image(systemName: icon(selected: selected))
								.foregroundStyle(selected ? Color.accentColor : .secondary)
							Text(repo.name)
							Spacer(minLength: 0)
						}
						.padding(.horizontal, 8)
						.padding(.vertical, 5)
						.background(repo == active ? AnyShapeStyle(.selection) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 6))
						.contentShape(.rect)
						.onTapGesture { click(repo) }
						.id(repo.path)
						.accessibilityElement(children: .combine)
						.accessibilityAddTraits(.isButton)
						.accessibilityValue(selected ? "selected" : "")
						.accessibilityIdentifier("\(identifierPrefix)\(repo.name)")
					}
				}
				.padding(4)
			}
			.frame(height: 180)
			.background(.quaternary.opacity(0.4), in: .rect(cornerRadius: 8))
			.onChange(of: active?.path) { _, path in
				if let path { proxy.scrollTo(path) }
			}
		}
	}

	private func icon(selected: Bool) -> String {
		switch mode {
		case .multi: selected ? "checkmark.square.fill" : "square"
		case .single: selected ? "largecircle.fill.circle" : "circle"
		}
	}
}
