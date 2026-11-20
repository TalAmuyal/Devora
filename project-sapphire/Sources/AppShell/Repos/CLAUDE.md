# Repos

Ember's Add Repo and Clone Repo dialogs (`Sources/AppShell/Repos/`), modal layers over the window, each with its form, then the operation's progress in its place (`RepoOperationPhase`, as Ember's `driveCreationDialog`).

The repo picker of the New Agent, New Task and Add Repo dialogs is one native list (`RepoListView`, a bordered SwiftUI `List`): the cursor row is the list's selection, the dialog passes its height, long names truncate in the middle with the full name as the tooltip, and no match shows the system's empty-search view.
In New Agent and New Task each repo is a checkbox: the box or its name toggles it and makes it the cursor row, and a click on the row's empty area only moves the cursor (`activate`); in Add Repo the selection is the chosen repo.

- Clone Repo (`CloneRepoDialogModel`, `Surfaces.openCloneRepoDialog`) opens from the New Task dialog's "+ Clone Repo", cloning into the selected profile; its "Clones to:" hint is the core's `clone_destination`, which parses the URL as the clone does.
	Once cloned, the New Task dialog lists the profile's repos again with the clone selected, keeping its title and selection (`NewTaskDialogModel.reposChanged`, Ember's `refreshAfterClone`).
- Add Repo (`AddRepoDialogModel`, `Surfaces.openAddRepoDialog`) adds a worktree of one of the profile's repos to a workspace, as `<repo>` or `<repo>-<postfix>` (`AddRepoTarget.worktreeDirName`); the core rejects a name the workspace has or one that leaves it, as the add's failure.
	It takes an `AddRepoTarget`: the workspace and its own profile, whose repos it lists and whose prepare-command runs, whichever profile is selected.
	The Command Palette opens it on the active tab's workspace (`Store.addRepoTargetOfActiveTab`, which refuses a tab without a workspace with Ember's banner); the Workspace Hub's Add Repo opens it on the focused workspace, then reloads that workspace's status.
- Their keys are Ember's: Escape cancels and Enter confirms whatever has the focus, `q` cancels outside a field, Tab and Shift+Tab cycle the fields and buttons, and Space presses the focused button; during the progress, Escape, `q` and Space press its Cancel, or Close once it has failed.
- `Store.startRepoOperation` starts either (`add_repo_to_workspace`, `clone_repo_into_profile` in `devora-ffi`'s `repos.rs`) and follows its progress into a `RepoOperationRun`; done or cancelled closes the dialog, and a failure, which also has its banner, stays in it until it is closed.
	A cancel is `cancel_workspace_creation` (the operations' ids are a creation's), held until the id has arrived; the core undoes the work (the added worktree, the partial clone) before it reports the cancel.
