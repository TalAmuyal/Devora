# Settings Hub

Ember's Settings Hub (`Sources/AppShell/Settings/`): the profiles beside the focused row's detail, on a page layer over the Workspace Hub (`Surfaces.openSettingsHub`), so that closing it (`q`, Escape, or what closes it below) reloads the hub.

- `SettingsHubModel` holds one opening: the master list ("User Defaults" pinned first, the store's profiles with the active one's dot and their repo counts, "New Profile…" pinned last), its cursor, and the focused row's detail; it lists the profiles again on opening (`Store.listProfiles`) and focuses the active profile's row, or the New Profile row for "New Profile…".
	Its keys are Ember's: `j`/`k` and the arrows move, Enter sets the focused profile active, `n` (or Enter on the New Profile row) gives the form's name field the focus on the main actor's next turn, and `d` deletes; a key typed into a card's input or the form's fields is theirs (Escape leaves the field), and `q` and Escape go on to the stack's dismissal.
- A profile's detail: its path, its repo, workspace and active-task counts (`get_registered_repos` and `list_workspaces` the first time its row is focused), Set Active, Clone Repo (the Clone Repo dialog, after which the counts and repos load again) and Delete, and its Repos card.
	Set Active and registering a new profile make it the core's active profile at once (`Store.activateProfile`) and close the Settings Hub; the Workspace Hub's reload then lists its workspaces, starting its cursor over.
	Delete asks with Ember's confirmation (the directory "remains on disk"), or shows a notice (`Confirmation` with no `cancelLabel`) listing the open tabs that use the profile (`Store.tabs(usingProfile:)`); `Store.unregisterProfile` (`unregister_profile`) then loads again, which selects the first profile if the deleted one was selected, and deleting the last one closes the Settings Hub over the first-run welcome.
- The New Profile row's detail is the profile form, whose Cancel goes back to the active profile's row; Tab and Shift+Tab move through it while it has the focus.
- The User Defaults' and each profile's detail have Claude Launch Settings, Terminal & Session, Pull Requests and Task Tracker cards; the Asana settings and token row show while the provider resolves to Asana, an inherited one included.
	`ClaudeSettingsModel` edits the three model tiers and permission mode as Custom/Default/None, effort as Default/levels/None, and Default model as On/Off (On stores `opusplan`, Off stores explicit None).
	Custom reveals a draft seeded from the resolved value without writing; Enter or Save commits trimmed text, a blank commit inherits again, and drafts survive unrelated writes.
	The core exports the effort levels and model/permission suggestions from `devora-config`; neither UI owns a copy of those lists, and suggestions never restrict custom ids.
	`SettingsWriteQueue` serializes generic and Claude writes together because both update the same config file.
	`AsanaTokenModel` reads only presence and clears its secure draft after a successful save; the credential is shared by both scopes and Debi, stored as service `devora-asana` for the login account.
	The acceptance harness's `DEVORA_TEST_MODE=1` selects the core's in-memory credential backend in both Sapphire and Ember; model tests inject closures and Rust tests inject a `TokenStore`, so tests never read or modify the real login keychain.
	`ConfigSettingsModel` is Ember's `settingsEditor` for one scope: it reads the scope (`get_config_settings`, the stored and the resolved values), writes one key at a time, in order (`set_config_setting`, where nil removes the key), and reads the scope again after each write.
	"Default" removes the key, and shows the inherited value as a hint ("→ value", "Uses Devora default" when nothing applies; not for `pr.auto-merge`, whose per-repo override the hub can't see).
	A text setting's Set reveals an input and focuses it, writing nothing; Enter or its Save button (shown only for a non-blank value that differs from the stored one) commits, trimmed except for the title prefix, and a blank value reverts to Default; what is typed stays across the reloads of other settings' writes, and comes back when Set is selected again.
	The Agent card's `AgentSettingsModel` shares that write queue and scope reload. It edits compaction percentage, a template path with a file picker, multiline role/user-details/custom-sections and literal key/value template inputs. The existing Enter sends control is in the same card. Multiline Enter stays text and Save preserves whitespace and explicit empty strings. Default removes an override. Core-returned sources label effective values, and explicit legacy nulls remain visible as cleared overrides. Each input rename is one core write, and its row is disabled while its write is pending. A legacy null inputs map can be reset to Default before editing entries.
	A profile's Agent card previews saved instructions through `agent_creation_preview` with an empty form, coding instructions included, and that card's owning profile. Each successful queued write rereads the settings and refreshes the preview. Unsaved drafts stay local. Preview failures are inline, and an older render cannot replace a newer reload. User Defaults points to the profile cards rather than rendering a prompt without a profile.

## Repository Updates

The Repository Updates card appears only in User Defaults.
It edits the shared core's enabled flag and positive interval in minutes, with Sapphire's enabled default shown in the inherited hint.
Interval values are edited as text and stored as JSON numbers by the core.
Run Repo Updates in the palette requests a manual round even with the periodic switch off.
It uses the same Global Pause boundary as automatic rounds.
The launch starts the single core worker, and the off-main termination closure shuts it down before awaiting the agents.
Operational outcomes go to the app log without banners.
