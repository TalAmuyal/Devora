# Profiles and the first run

Ember's profile form and first-run welcome (`Sources/AppShell/Profiles/`).

- The profile form (`ProfileFormModel`) has a name and a root path, with "Browse…" (an `NSOpenPanel` folder picker); it validates the path (`validate_profile_path`) 250 ms after the last edit, or at once when it is set (the initial path, a browsed folder), as a new profile, an existing one (whose own name locks the name field, and "Register Profile"), or an invalid path.
	Submitting registers it (`register_profile`, which creates `config.json`, `repos/` and `workspaces/` for a new one), then the hub selects it and loads it from scratch.
	Enter submits from a field or the submit button, Space presses the focused button, and Tab and Shift+Tab move through it, skipping a locked name and a disabled submit; it draws its buttons' focus itself, as the dialogs do.
- With no profiles, a listing that succeeded makes the Workspace Hub show the first-run welcome (`WorkspaceHubModel.welcome`) instead of its list: the profile form, on `~/devora`, with no control focused, as in Ember.
	The hub refuses `q` and Escape meanwhile (`dismissDecision`), as Ember's zero-profile lock does; Escape in a field leaves the field, and the hub's own keys do nothing.
	The burger menu has no "Settings" and the header no profile dropdown, as in Ember.
- The selected profile is the core's active profile (`AppState.activeProfile`, which `Store.selectedProfilePath` reads): selecting one dispatches `SetActiveProfile`, so a launch restores it, where Ember always starts on the first profile.
	A load selects the restored profile while it is registered, otherwise the first profile, or none, and dispatches that in its place; it reads the active profile from the core, since the state's events fold on a later frame, and folds its own selection at once, so that the New Task dialog and the hub see it as soon as the workspaces are listed.
