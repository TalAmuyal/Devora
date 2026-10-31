import CoreBindings

/// Whether Remove Task can go ahead at once, as Ember's hub decides: only when every repo is known to be clean and on a detached HEAD, since removing a task discards the changes and detaches the branches.
enum RemoveTaskDecision: Equatable {
	case removeNow
	/// The repos that need cleanup; empty when the status isn't known (still loading, or unreadable).
	case confirm(problems: [Confirmation.Item])

	init(status: WorkspaceStatus?) {
		guard let status, status.error == nil else {
			self = .confirm(problems: [])
			return
		}
		let problems = status.repos.compactMap { repo -> Confirmation.Item? in
			var reasons: [String] = []
			if repo.modified > 0 || repo.untracked > 0 { reasons.append("uncommitted changes (will be discarded)") }
			if !repo.isDetached { reasons.append("on branch '\(repo.branch)' (will be detached)") }
			return reasons.isEmpty ? nil : Confirmation.Item(name: repo.name, detail: reasons.joined(separator: "; "))
		}
		self = problems.isEmpty ? .removeNow : .confirm(problems: problems)
	}

	/// The dialog that asks before removing the task of `workspace`.
	static func confirmation(for workspace: Workspace, problems: [Confirmation.Item]) -> Confirmation {
		Confirmation(
			title: "Remove task from \"\(workspace.id)\"?",
			intro: problems.isEmpty
				? "The repos' status isn't known, so any uncommitted changes will be discarded and any branch detached."
				: "The following repos need cleanup:",
			items: problems,
			warning: "This cannot be undone.",
			confirmLabel: "Remove Task")
	}
}
