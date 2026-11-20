# Workspace Hub

Ember's home page (`Sources/AppShell/WorkspaceHub/`): the selected profile's workspaces, beside the focused one's details, on a page layer over the whole window.

- `Surfaces.openWorkspaceHub` pushes it with a fresh `WorkspaceHubModel`, which holds what one opening has (the category, the filter, the cursor, the toast), so closing the hub forgets them, as Ember's `unload` does.
	The profiles, the workspaces and their statuses are the store's; the hub loads them on opening, reloads them silently when a page over it closes (its `onReveal`), and reloads them with a toast on `R` ("Refreshing…" for at least 1 s, then "Refreshed successfully" for 3 s unless a call failed; the latest press wins).
- The statuses (`Command.loadWorkspaceStatuses`) are one `workspace_status` core call per workspace, off the main actor and four workspaces at a time (each runs its repos' `git status` in parallel, at most five at once), the active workspaces first; each is kept as it arrives, so the list and the detail panel fill in progressively, and a workspace without one shows as pending.
	A workspace whose status can't be read is invalid (an error dot, badge and message), with no banner; a failed call also has its banner.
	Dropping the statuses (a fresh load, another profile) makes the loads still running drop theirs.
- Its keys are in `WorkspaceHubModel.handleKey`; while the filter field has the focus, keys type into it and Escape leaves it, as Ember's editable-element guard does.
	The cheatsheet (`?`) is a page of its own over the hub, scrolled by `VimScroll`, ADR-004's motions over a SwiftUI scroll view; `ListCursor` is the list's cursor.
- The detail panel's buttons are Ember's: Open; Duplicate and Remove Task for an active workspace; Delete for an inactive or invalid one.
	An active workspace also has Add Repo, a Sapphire addition alongside the Command Palette action (`../Repos/CLAUDE.md`).
	Remove Task (`Store.removeTask`, `remove_task`) goes ahead at once when every repo is known to be clean and on a detached HEAD (`RemoveTaskDecision`), and otherwise asks with a confirmation dialog listing what will be lost; Delete (`Store.deleteWorkspace`, `delete_workspace`) never asks.
	Either one lists the workspaces again, keeping the filters and the cursor; a failure shows its banner.
- The confirmation dialog (`ConfirmationDialog.swift`, `Surfaces.confirm`) is a modal layer over the window, with Ember's keys: Escape and `q` cancel, Enter confirms, Tab and Shift+Tab move between its two buttons without ever leaving it, and Space presses the focused one; it draws its own focus, so the Tab trap doesn't depend on the system's keyboard navigation setting.
- The header has Ember's two dropdowns (`DropdownMenu.swift`): the profiles (each with its repo count, the selected one checked), and the burger menu, whose "Save loading latencies" writes the latest load's `LoadingLatencies` to `<profile>/diagnostics/hub-profile-<timestamp>.json` and shows the path in a toast.
	An open dropdown is a popup layer (`Surfaces.toggleHubMenu`) whose content is empty: the hub draws the panel under its trigger, as Ember's caller-mounted popup, with a click anywhere else closing it.
	Escape and `q` close only the dropdown; any other key closes it and reaches the hub.
- `P`, the profile dropdown's "Manage Profiles…" and the burger menu's "Settings" open the Settings Hub, and "New Profile…" opens it on its New Profile row (`../Settings/CLAUDE.md`).
- `H` and the burger menu open the Health Hub as a window-stack page. It runs the shared core health check off the main actor, cancels it on close or re-run (`r`), supports Vim scrolling, and copies fix hints with a toast.
