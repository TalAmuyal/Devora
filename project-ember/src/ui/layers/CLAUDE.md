# Layer System — current reference

Stacked UI surfaces (pages, modals, popups, session panels) are routed and focused by **their position in the `LayerStack`**, never by listener-registration order or by whatever happens to hold focus.

This file is the cross-cutting reference for behavior that spans several files.
Knowledge that one file owns is documented in that file — see [Where things live](#where-things-live).
For the history of *why* this system exists, see [ADR-003](../../../../docs/adrs/ADR-003-ember-layer-system.md).

## Mechanism vs. policy (seam)

- **`LayerStack.ts` is the mechanism** — stack ordering, wrapper mounting, focus resolution, the single `window`-capture keydown dispatcher, and barrier semantics. It is **domain-free**: it imports only `./types`, `./tabTrap`, and `../focus`.
- **`../KeyboardShortcuts.ts` is the policy** — the concrete app-wide shortcut set (F1, `Ctrl+S`, Shift-Shift, etc.). It depends on `SessionManager`.

They are wired together at the composition root (`src/main.ts`) via `setKeyObserver`, `setGlobalHandler`, and `setPageBarrierAdmits`.
`LayerStack` never imports `KeyboardShortcuts`.

The seam keeps domain code out of the generic mechanism and lets `LayerStack` be unit-tested with a fake admit-list (see `__tests__/LayerStack.test.ts`) while the shortcut policy is tested standalone.

## Key dispatch, in brief

One `window`-capture keydown listener walks the stack top-down.
Each layer gets `onKey` first, then the dismiss-key check; `page` and `modal` are opaque barriers that **end the walk**, while `popup` and `panel` pass unhandled keys down.
Keys that reach the base go to the app-wide shortcut handler.
Consuming a key uses `stopImmediatePropagation` (not `stopPropagation`) so no sibling window listener can also act on it.

`LayerStack.handleKeydown` is the canonical definition — read it if more detail is needed.

## Modality-barrier matrix

Which keys survive each barrier.
The page admit-list is owned by `KeyboardShortcuts.allowedThroughPageBarrier` (the single source of truth for "app-wide" keys) and consulted by `LayerStack.allowedThroughBarrier`, which additionally blocks everything under a `modal`.

| Shortcut | base (empty) | under panel/popup only | under page | under modal |
|---|---|---|---|---|
| `q` / Escape | → focused element | dismiss top layer | route via `onUserDismissRequest` | dismiss (cancel) via the modal's `onKey` |
| `Ctrl+S` (Workspace Hub) | open hub | open hub | **dismiss key** (hub: close; Settings: pop to whatever's beneath; palette: no-op via its `onKey`) | blocked |
| `F1` (User Guide) | open guide | open guide | **allowed** — pushes the guide over the page | blocked |
| `Ctrl+Shift+S` (new session) | allowed | allowed | **blocked** | blocked |
| `Ctrl+←/→` (switch tab) | allowed | **allowed** | **blocked** | blocked |
| `Ctrl+Shift+←/→` (move tab) | allowed | allowed | blocked | blocked |
| Font size (`Ctrl+1/2/3`, `Ctrl±`) | allowed | allowed | **allowed** | blocked |
| Shift-Shift (palette) | allowed | allowed | blocked (guard: no page/modal on stack) | blocked |

The blocked cells under a page are deliberate, the reasoning lives with the admit-list in `KeyboardShortcuts.allowedThroughPageBarrier`.

## When a sub-view is its own layer

Because routing and focus follow stack position, a sub-view that **fully covers** the surface beneath it and **owns its keys** must be pushed as its own layer — otherwise the host keeps routing keys to the now-hidden view (a within-surface leak).

- Use a **`modal`** when the surface below stays as visible context (e.g. the hub's "New Task" form, Add-Repo, etc.).
- Use a **`page`** when it is a full-window replacement (e.g. the hub's keyboard cheatsheet).
- A *mode with nothing behind it* (the zero-profile welcome) or a *master/detail split where both panes stay live* (Settings Hub) is **not** a covering layer: it stays part of its host, guarded by its own state.

## Modals and pages treat Escape oppositely

**Modals consume Escape; pages pass it through.**
The mechanics of the modal side live in `../components/ModalDialog.ts` — this section records only why the two disagree.

A modal must consume it because dialogs focus a plain `<input>` with no element-level Escape handler, so routing Escape through the central "editable focused → pass through" guard would make it a dead key.
A page must pass it through so a focused search field can blur on the first press and the page dismisses on the second.

Both surfaces keep `q` on the central guard, so it types while an input is focused.
This is also why the Command Palette needs no special case: its search field is focused on open, so `q` types and Escape reaches the field's own handler.

## What stays outside the layer system

### Toasts and error banners

These are passive elements on `document.body` above everything; they take no focus and never participate in routing.

### Crit panel's `<iframe>`

Once the user clicks *into* it, keydown events fire in the iframe's document and never reach the host window, so layer routing (and every global shortcut) is inert until focus returns to host chrome.
This is a browser constraint the layer system cannot change; the panel takes focus on activation so `q`/Esc works until the user deliberately clicks inside the iframe.

## Where things live

| Knowledge | Canonical home |
|---|---|
| Layer kinds, the `LayerSpec` contract (`onKey`, `onUserDismissRequest`, etc.), dismiss decisions | `types.ts` |
| Dispatch algorithm, focus lifecycle (push/pop/reveal/replaceTop), barrier evaluation | `LayerStack.ts` |
| Tab focus trap | `tabTrap.ts` |
| Process-wide singleton accessor | `stack.ts` |
| App-wide shortcut policy + the page admit-list | `../KeyboardShortcuts.ts` |
| How a modal handles Escape/Enter, backdrop routing, the Tab trap | `../components/ModalDialog.ts` |
| Why this system replaced the previous overlay handling | [ADR-003](../../../../docs/adrs/ADR-003-ember-layer-system.md) |
