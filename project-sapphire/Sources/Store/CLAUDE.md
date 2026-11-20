# The store and the app state

- The tabs are core-owned state (`devora-state`, ADR-008): the store dispatches a `StateCommand` to the core and changes `Store.app` only by folding the core's events.
	The one early fold is a profile selection's own `ActiveProfileChanged`, applied as soon as `SetActiveProfile` succeeds so that the hub and the New Task dialog see it once the workspaces are listed; the core's copy of that event folds later and sets the same value.
	It waits until the subscription's first event, the core's snapshot, has folded, folding it at once rather than on a frame: the snapshot replaces the whole state, so folding it after the early fold would undo the selection until the core's event arrived.
- The core commits every command to `state.db` (SQLite) before its events go out, in `~/Library/Application Support/Devora Sapphire/`, or `Devora Sapphire <suffix>/` for a bundle with an install suffix; `DEVORA_STATE_DIR` replaces the directory (tests).
	The core resolves that path (`sapphireStatePath`, from the bundle's `Resources`); `Store(configPath:state:log:)` takes `.app` for it, `.file` or `.inMemory` (tests).
	A failed command, including one that couldn't be saved, takes no effect and shows as an error banner.
- The app state has one writer: the core holds `state.db` exclusively while it runs, so a second instance on it gets `StartupProblem.stateInUse` and can only quit.
	A state that can't be opened (corrupt, from a newer build) is `StartupProblem.stateUnreadable`: Quit leaves it untouched, Start Fresh (`Command.startFresh`) renames it, with its WAL, to `state-unreadable-<UTC time>.db` beside it and opens a fresh one.
	Until the problem is answered, the store has no core and `StartupProblemView` fills the window.
- The launch (`RootView`) runs once the state is open: the profiles and the selected profile's workspaces, then the Workspace Hub if the core has no tabs (`Store.hasNoTabs`, read from the core since the restored tabs fold on a later frame); restored tabs get fresh terminals from their specs (a terminal's contents aren't restored), and a workspace creation the app stopped in the middle of comes back as failed ("interrupted").
- The store pulls the core's events through `Batches`, the `AsyncSequence` over a core subscription.
	Long-lived state/error consumers resolve their weak store inside the main-actor hop, so queued delivery cannot keep a released store and its database alive. Cancelling the consumer closes its subscription through `Batches`.
	Events wait in an off-main inbox until `FrameDriver`'s display link folds them into `Store.app`, once per frame and only while events are pending.
	Without a display link (tests), they fold on the main actor's next turn.
