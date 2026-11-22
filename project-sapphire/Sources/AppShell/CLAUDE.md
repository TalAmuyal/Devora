# AppShell

## Terminals and surfaces

- `Terminals` (in `AppShell`) holds the live terminals, keyed by the terminal id of each tab's pane: it makes one when a tab appears and drops it when the tab leaves the state, which frees the libghostty surface and ends its process.
	The terminal is at the bottom of its tab's stack at once, while its launch (the core's `terminalSpec`) loads; it runs once it has its launch and its view is in the window, when libghostty makes the surface and spawns the shell.
	What is typed into it before then (keys, the bridge's lines, a paste) waits in its `TerminalSession` and goes to the surface in order once it exists; the PTY holds it until the shell reads it, as in any terminal.
	Every tab's terminal stays in the view tree, so switching tabs keeps them running; a terminal whose process exits closes its tab, as in Ember.
	A tab creating its workspace has no terminal until the workspace exists (its session is `CreatingWorkspace`, which has no terminal spec).
- `Surfaces` (in `AppShell`) holds the key router and one layer stack per tab, with the tab's terminal as its bottom layer, under the progress page of a tab creating its workspace; the active tab's top layer has the keyboard focus unless a window surface is open.

## Appearance

`AppAppearance` applies the User Defaults' Appearance (ADR-010) to `NSApp.appearance`: none on Automatic, so that the app follows the system, or aqua or darkAqua. `RootView.init` applies it before the window first shows (SwiftUI builds the window's content as the app finishes launching), so a user on Dark with a light system sees no light frame, and again whenever `Store.appearance` changes. Its `effective` appearance, light or dark, follows `NSApp.effectiveAppearance` (KVO), a system switch on Automatic included; it is what the panes Sapphire draws itself are to read. The terminals follow it too, through `TerminalRuntime.apply` (`RootView`).

## Terminal colors

The terminals draw in the palette chosen in User Defaults (`terminal.palette`: Selenized, the default, Catppuccin or Gruvbox; `Store.terminalPalette`, read as the appearance is), its light or dark variant after the effective appearance (ADR-010's amendment of 2026-11-21). `TerminalPalette.Family` holds the three, each tuned so that every text color reads at 4.5:1 on its page (`TerminalPaletteTests`, which names the background roles it exempts). `TerminalRuntime.apply` hands both variants and the effective appearance to TerminalKit, which switches the terminals' colors. The bridge's state has `appearance`: the setting, the effective appearance and the system's.

## Global Pause

The title-bar toolbar and command palette dispatch `Command.setGlobalPaused` through Store.
`AppState.globalPaused` is the core's durable, per-install hold, folded from the ordinary state subscription.
While held, the toolbar shows "Pausing… (n of m)" until every agent's runtime has stopped, then "Paused" (`Store.pauseIndicator`), from the core's `RunningAgents`, which counts an agent whose tab was closed mid-turn until its runtime stops.
The pause status and the Pause/Resume toggle are separate toolbar items, each in its own capsule; clicking the status opens the pause progress (`PauseProgress.swift`, `Surfaces.showPauseProgress`), an opaque native popover anchored to it that lists each agent still running and why (a turn, a permission question, stopping), with Show, which hides it and activates the agent's tab.
The popover leaves the window, and so the agents' permission cards, usable; Escape or a click elsewhere only hides it.
A quit (Cmd+Q, the bridge's quit, a system logout, or closing the window, which `WindowCloseQuits` routes through the quit) waits while an agent runs a turn or waits for an answer, or while the hold is still stopping someone (`Store.quitMustWait`): it holds Devora, shows the same popover with Cancel Quit (the default; the hold stays) and Interrupt and Quit, and quits by itself once no runtime is live (`Surfaces.finishPendingQuit`), even with the popover hidden.
Interrupt and Quit stops the agents at once, as quitting used to; a quit with nothing working is immediate.
What went wrong while stopping (a forced kill after the grace period, a repo update still running) is shown in the popover, whose Quit then quits; nothing is swallowed.
The agents are stopped by then, so the problems are kept (`Surfaces.shutdownProblems`): the toolbar's status reads "Stopped" and reopens them, with their Quit, until the app quits.
The hold stays committed after a graceful quit, so the next launch starts held (ADR-013).
An agent tab shows Pausing… until its current turn ends and Paused afterward, using the paused emoji and matching accessibility description.
`AgentChat.globallyPaused` disables sending and exposes the resume hint while drafts keep their normal persistence.
The core also refuses sends during the hold.
Permission responses and Stop remain available for a draining turn.
The chat's Keep paused switch sets the core-owned, generation-scoped release choice without changing the held worker.
The typed `globallyHeld` and `keepPaused` chat projections control its availability and selection.
The choice survives a relaunch while globally paused and can be reversed before Devora resumes.
The initial restored snapshot carries the global hold before agent restore launches work.
After a restart, an agent's first launch sends it a notice to re-check its waits, shown as a system row ("Devora restarted: asked the agent to re-check its waits", "(resent)" when resent after a crash); see `project-core/CLAUDE.md`.
