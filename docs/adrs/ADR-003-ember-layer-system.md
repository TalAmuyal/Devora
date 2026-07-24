# ADR-003: Ember Layer System (Stacked Pages & Pop-ups)

## Status

Accepted

## Date

2026-07-09

## Context

Devora-Ember stacks UI surfaces on top of the terminal: full-screen pages (Workspace Hub, Settings Hub, Health Hub, User Guide, Command Palette), modal dialogs (confirmation, text input, add-repo, clone-repo), an anchored dropdown menu, and per-session panel overlays (Crit review, task-creation progress).
These accreted one surface at a time, each bringing its own keyboard and focus handling.

Five uncoordinated mechanisms resulted:

1. `KeyboardShortcuts` — one `window`-capture keydown registered once at startup.
2. `OverlayManager` — a single tab-covering slot (a second show *replaces* the first) plus a per-session panel `Map`.
3. Per-hub `window`-capture keydown handlers added in each hub's `load()` and removed in `unload()`.
4. Per-dialog `document`-capture keydown handlers (four dialogs, as of this writing).
5. `DropdownMenu`'s own `document`-capture handler.

There is no stack data structure; "stacking" is emergent.
The DOM capture phase dispatches **window listeners (in registration order) → document listeners → target**, and `stopPropagation` does not silence sibling *window* listeners — only `stopImmediatePropagation` does.
The startup `KeyboardShortcuts` listener therefore always wins, its `stopPropagation` starves the dialogs' and dropdown's `document` handlers, and the hubs' later-registered window handlers keep firing regardless.

The consequences are a class of cross-layer defects:

- Escape/Enter on a confirmation dialog stacked over a hub operates the **hub underneath** (dismisses it, or activates a list row) instead of the dialog.
- Escape with a dropdown open **closes the whole hub** and leaks the dropdown's `document` listeners.
- Hub key handlers stay live under child dialogs — `d`,`d` in Settings stacks two dialogs; `j/k/n/R/P/H` operate the hub beneath a modal.
- `q` with the hub cheatsheet open **double-fires** (closes the cheatsheet, then closes the hub).
- Panel overlays advertise an Esc/q dismissal that is **dead** while the terminal has focus, because `showPanelOverlay` never takes focus and the xterm textarea matches the "is an editable element focused" guard.
- No dialog restores focus on close, and no Tab trap exists despite comments claiming "focus trapping".
- `Ctrl+←/→` tab switching is not gated by overlays, so switching under an open hub steals focus to the new terminal (hub keys go dead) and stales the focus target captured when the overlay opened.

Focus restoration compounds this: overlays capture an explicit `restoreFocusTo` at open time, so anything that changes the active session while an overlay is up restores focus to the wrong place.

We are about to add more pop-up kinds — additional modal forms, richer anchored popovers/menus, and **non-modal floating panels** (where the user keeps interacting with the content below while the panel is open).
Bolting these onto the current model would multiply the defect surface.
The shared failure is that key routing and focus depend on *what happens to be focused* and *the order listeners were registered*, rather than on *which surface is on top*.

## Decision

Introduce a single **`LayerStack`** (`src/ui/layers/LayerStack.ts`) that owns the only `window`-capture keydown listener for layer concerns and manages focus by stack position.
Every stacked surface becomes a *layer* declaring its kind, which fixes its stacking and modality behavior; nothing else registers a global key listener.
Routing walks the stack from the top down, and focus is resolved at each transition from stack state.
The stack is a domain-free **mechanism**: the app-wide shortcut **policy** is in `KeyboardShortcuts` and is injected at the composition root, so the barrier and the shortcuts it gates cannot drift.

The acceptance harness drives the same entry points as production — tests open and dismiss surfaces through the same code paths the app uses, rather than through a compatibility adapter that could let the tests exercise different wiring than users do.

## Consequences

- Key routing and focus become a function of stack position, not of registration order or incidental focus. The seven cross-layer defects above are fixed structurally rather than patched per surface.
- New pop-up kinds are added by pushing a layer of the right kind; the four kinds already cover modal forms, anchored popovers, non-modal panels, and full pages, so the common cases need no new routing code.
- Some navigation becomes true stacking and changes visibly (intended): `F1` and Health/Settings opened from the hub now stack over it and reveal it on dismissal instead of replacing it; `Ctrl+Shift+S` and `Ctrl+←/→` are inert while a page is open; `q` now cancels modals and closes dropdowns (completing the vim-style dismissal convention). These are pinned by new acceptance scenarios.
- `OverlayManager`, the per-hub and per-dialog key listeners, the `DropdownMenu` key listener, the `commandPaletteOpen` flag, and two `main.ts` monkey-patches are removed. The single dispatcher plus per-layer specs replace them.
- Focus behavior inside an iframe-backed panel remains bounded by the browser; this is documented rather than worked around.
