# ADR-003: Ember Layer System (Stacked Surfaces)

## Status

Accepted

## Date

2026-07-09, revised 2026-07-29

## Context

Devora-Ember stacks UI surfaces: full pages (Workspace Hub, Settings Hub, Health Hub, User Guide, Command Palette), modal dialogs, an anchored dropdown, and per-session surfaces (Crit review, task-creation progress) over a terminal.

These accreted one surface at a time, each bringing its own keyboard and focus handling, and five uncoordinated mechanisms resulted: a `window`-capture listener registered at startup, an `OverlayManager` with a single tab-covering slot, per-hub `window` listeners added in each `load()`, per-dialog `document` listeners, and the dropdown's own `document` listener.

There was no stack data structure; "stacking" was emergent.
Because the DOM capture phase dispatches **window listeners (in registration order) → document listeners → target**, and `stopPropagation` does not silence sibling *window* listeners, the startup listener always won, starved the dialogs' and dropdown's handlers, and the hubs' later-registered handlers kept firing regardless.

The consequences were a class of cross-layer defects: Escape on a dialog operated the hub underneath; Escape with a dropdown open closed the whole hub and leaked its listeners; hub keys stayed live under child dialogs (`d`,`d` stacked two dialogs); `q` with the cheatsheet open double-fired; a session panel's advertised Esc/`q` was dead while the terminal held focus; no dialog restored focus and no Tab trap existed; and `Ctrl+←/→` was ungated, so switching tabs under an open hub stole focus and staled the overlay's captured focus target.

The shared failure was that key routing and focus depended on *what happened to be focused* and *the order listeners were registered*, rather than on *which surface is on top*.

### What the first expression of the fix got wrong

Routing and focus by stack position held.
Three things about how it was expressed did not, and each had to be undone:

- **One stack for surfaces that are not all app-wide.** Tab-bound surfaces had to live in a window-global stack, which required a hand-rolled emulation of the missing concept: a throwaway "active-panel sentinel" element created and destroyed on every tab switch, a bottom-insertion special case in `push`, a matching one in the reveal path, and a `reconcile()` keeping "exactly one sentinel iff the active session has a panel" true. Four mechanisms compensating for one absent idea — *this surface belongs to a tab, not the window*. It also leaked: closing a tab left its panel behind, because the panel was mounted outside the tab's container.
- **Surfaces ranked by archetype rather than by position.** A `LayerKind` enum (`page | modal | popup | panel`) decided modality, focus, paint order, and mounting together. Because a kind fixed paint order, the CSS carried a hand-maintained ladder (`.overlay-panel` 50 < `.overlay-tab-covering` 100 < toast 200 < modal backdrops 9999 < banner 10000) that was correct only because everything happened to share the root stacking context — two stacked pages both got `z-index: 100` and painted correctly only because append order happened to match stack order.
- **Surfaces deciding which app-wide keys survive them.** Keys reached the app-wide shortcuts only after getting past every barrier, gated by an admit-list held separately from the shortcuts themselves — a drift risk the code commented on twice. It also let a surface shadow an app-wide key by accident: the Workspace Hub claims bare `1`/`2`/`3` as category filters with no modifier guard, and its `onKey` ran before the barrier check, so `Ctrl+1` could never reach font sizing. `Ctrl+S` had grown an undocumented second job as a dismissal key for any page, and a modal blocked *everything*, which made font sizing inert under a dialog — an accident of "modals block", not a decision.

The trigger for undoing them was wanting the terminal itself to be a layer, so a CLI app (`claude`, `top`) can open as a surface over a shell.
That is not expressible while the terminal is a special "base" case beneath the stack, and it needs a combination of behaviors — opaque, but non-dismissible and consuming nothing — that no archetype provides.

## Decision

Route and focus every stacked surface by **its position in a stack**, and derive its extent from **which stack it is in**.
Nothing else registers a global key listener, and no surface carries a `z-index`.

1. **One stack per tab, plus one for the window.** A surface's extent stops being something it declares: the window stack covers the app, a tab's stack covers its session. Each stack mounts into a single host element, so hiding a tab hides its whole stack.
2. **The mechanism knows no archetypes.** A layer declares behavioral properties; the familiar archetypes survive as authoring presets, so unusual-but-valid combinations (a terminal; a non-modal find bar over a live terminal) need no new kind.
3. **Paint order is DOM order.** Wrappers are appended in stack order and isolated, so a surface cannot be raised visually without being raised in its stack. Toasts and error banners stay outside the system.
4. **Keys are routed shortcuts-first**: app-wide shortcuts → window stack → active tab stack. Each shortcut states its own availability, so no admit-list exists to drift. A stack that reports a blocked key stops routing **without** consuming the event, so the key still reaches the focused element — which is how a terminal layer receives typing while remaining an opaque barrier.
5. **Mechanism and policy stay separate**, wired at the composition root: the stack and router are domain-free, and the app-wide shortcut set lives with the domain.

Two shortcuts change deliberately as a result: **`Ctrl+S` opens only** and is never a dismissal, and **`F1` is unconditional** except when the guide is already open.

The current contract — the routing table, shortcut availability, the presets, and when a sub-view needs to be its own layer — is documented where it is implemented; see [`project-ember/src/ui/layers/CLAUDE.md`](../../project-ember/src/ui/layers/CLAUDE.md). This record deliberately does not restate it.

## Alternatives considered

- **Keep the five ad-hoc mechanisms and patch each defect.** Rejected: the defects are a class, not a list; every new surface would re-open it.
- **Keep one stack and add tab-awareness to layers.** Rejected: it keeps the four compensating mechanisms and merely renames the sentinel. The topology *is* the missing concept.
- **Assign `z-index` from the stack index, with reserved bands above.** Correct, but pure bookkeeping — a reindex after every push, remove, and `replaceTop`, plus a forced re-banding of toasts. One host per stack gets the same guarantee structurally.
- **A small set of fixed `z-index` levels** (`surface-back`/`surface-front`). Rejected: it cannot express two stacked pages, and `F1` over a dialog needs a "back" surface above a "front" one.
- **Collapse the archetypes all the way, presets included.** Rejected: six flags at every call site invites inconsistent surfaces. The presets are the readable form; the mechanism just should not depend on them.
- **A UI framework, or `<dialog>`/`popover`.** Rejected: the problem is routing, modality, and focus; frameworks solve rendering, and adopting one would rewrite ~30 component factories and the acceptance-test DOM contract while leaving the actual subject unchanged. `<dialog>` would need its own Escape and focus behavior neutralised so it does not fight the router, and its main draw — top-layer painting with no `z-index` bookkeeping — is what one host per stack already delivers.
- **A positioning library (`@floating-ui/dom`) or `focus-trap`.** Deferred, not rejected: there is exactly one anchored surface today, and the Tab trap is 49 lines. Revisit at a third anchored surface.

## Consequences

- The seven cross-layer defects are fixed structurally rather than patched per surface, and routing no longer depends on registration order or incidental focus.
- Adding a surface is two decisions — which stack, and which preset — instead of picking an archetype and inheriting its unrelated baggage. Opening a CLI app as a surface becomes a push of a terminal layer onto a session's stack.
- Deleted outright: `OverlayManager`, the per-hub/per-dialog/dropdown key listeners, the `commandPaletteOpen` flag, two `main.ts` monkey-patches, the panel sentinel and its `reconcile()`, `SessionPanels`, the "base focus" dependency reaching back into `SessionManager`, the shortcut admit-list, and every `z-index` in the layer CSS.
- User-visible changes, all intended: `F1` and Health/Settings stack over the hub and reveal it on dismissal instead of replacing it; `q` cancels modals and closes dropdowns; font sizing works inside the hub and under dialogs; `F1` opens the guide over a dialog; `Ctrl+S` no longer closes anything; a Crit review no longer participates in window-level routing at all. Each is pinned by an acceptance scenario.
- Closing a tab now tears its surfaces down, because they live inside it.
- What this does **not** fix: focus inside the Crit review's cross-origin `<iframe>` still makes every shortcut inert, because those events never reach the host window. That is a browser constraint needing its own fix.
