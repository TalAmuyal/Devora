# Layer System — current reference

Stacked UI surfaces are routed and focused by **their position in a stack**, and their extent comes from **which stack they are in**.

This file is the cross-cutting reference for behavior that spans several files.
Knowledge that one file owns is documented in that file — see [Where things live](#where-things-live).
For *why* the system looks like this — what it replaced, and what was considered and rejected — see [ADR-003](../../../../docs/adrs/ADR-003-ember-layer-system.md).
This file is the current contract; the ADR deliberately does not restate it.

## Topology: n + 1 stacks

- **The window stack** (`stack.ts`, hosted on `#app`) holds surfaces that cover the app: hubs, the User Guide, the Command Palette, etc.
- **One stack per tab**, owned by its `SessionTab` and hosted on that tab's container.

A surface is app-covering **iff** it is in the window stack.
Nothing else declares extent, and the same `pageLayer` preset produces a full-window hub or a session-sized Crit review depending only on where it is pushed.

Two consequences worth knowing: hiding a tab hides its whole stack (a live `<iframe>` survives a tab switch untouched), and closing a tab tears its surfaces down with it.

Only the window stack has a singleton accessor (`getWindowLayerStack()`), because deep call sites like `showModalDialog` cannot be threaded an instance.
A tab stack is always passed explicitly.
That is also why **modals and dropdowns are window-level**: every dialog today is raised over a hub, and every dropdown is anchored inside one. <- This line should be challenged at some point - could a dropdown or a dialog be tab-level? If so, the system would need to be extended to support that.

## Mechanism vs. policy (seam)

- **`LayerStack.ts` is the mechanism** — ordering, mounting, focus resolution, dismissal, and what a key walk reports. Domain-free: it imports only `./types`, `./tabTrap`, and `../focus`.
- **`LayerRouter.ts` is the wiring** — the single `window`-capture keydown listener and the three-stage routing order. It knows about stacks, but not about surfaces.
- **`../KeyboardShortcuts.ts` is the policy** — the concrete app-wide shortcut set and each shortcut's precondition.

`main.ts` wires them via `setKeyObserver` and `setShortcutHandler`.
Neither `LayerStack` nor `LayerRouter` imports `KeyboardShortcuts`.

## Key dispatch, in brief

One listener runs three stages in order:

1. **App-wide shortcuts**, run *first*, so no surface can shadow them by accident and no surface decides which of them survive it.
2. **The window stack**, walked top-down.
3. **The active tab's stack**, reached only if the window stack passed the key on.

A stack walk reports one of three things, which is the whole routing contract:

| Result | Meaning | Event |
|---|---|---|
| `consumed` | a layer acted on it | `preventDefault` + `stopImmediatePropagation` |
| `blocked` | an opaque layer ended the walk without acting | **left alone**, so it reaches the focused element |
| `passed` | nothing claimed it | routing continues to the next stack |

`blocked`-without-consuming is what makes a terminal layer work: it is an opaque barrier, but typing still reaches xterm's hidden textarea.

`LayerStack.route` and `LayerRouter.handleKeydown` are the canonical definitions — read them if more detail is needed.

## Shortcut availability

Every app-wide shortcut states its own precondition in `KeyboardShortcuts`.

| Shortcut | Available |
|---|---|
| Font size (`Ctrl+1/2/3`, `Ctrl±`) | Always — a display preference no surface has a reason to disable |
| `F1` (User Guide) | Always, except when the guide is already open |
| `Ctrl+S` (Workspace Hub) | When no opaque layer is in the window stack |
| `Ctrl+Shift+S` (new session) | When no opaque layer is in the window stack |
| `Ctrl+←/→` (switch tab), `Ctrl+Shift+←/→` (move tab) | When no opaque layer is in the window stack |
| Shift-Shift (Command Palette) | When no opaque layer is in the window stack |

A shortcut whose precondition fails is still **consumed**, as a deliberate no-op — `Ctrl+S` with the hub open must not fall through to the hub's own key handling, and none of these keys should ever reach a terminal.

Shift-Shift is the one exception to "shortcuts run in the keydown router": its second tap resolves on **keyup**, on a listener `KeyboardShortcuts` owns directly, so its precondition is checked there.

## Paint order

Wrappers are appended to their stack's single host in stack order, so **paint order is DOM order** and no layer carries a z-index.
Each wrapper is `isolation: isolate`, so z-indexes used *inside* one surface cannot escape and outrank the surface above it.
A tab's host sits inside `#main-panel` (also isolated) and therefore below every window layer.

The invariant this buys: **a surface cannot be raised visually without being raised in its stack.**

Toasts and error banners stay outside the system on `document.body`; their positive z-indexes are what keep them above every layer, and that stays true at any stack depth.

## Presets, and when to reach past them

`presets.ts` bundles the behavioral flags into the four combinations in use.
Use these over the raw flags whenever possible.

| Preset | Use for | What is special |
|---|---|---|
| `pageLayer` | hubs, the guide, the palette, a Crit review | dismissing it refreshes what it covered (navigation) |
| `modalLayer` | dialogs | traps Tab; dismissing it does *not* refresh the caller |
| `popupLayer` | anchored dropdowns | takes no focus, caller-mounted, transparent to keys, auto-closes with its host |
| `terminalLayer` | the bottom of a tab stack | opaque, **not** dismissible, consumes nothing |

The flags exist for combinations that have no preset yet — the clearest being a **non-modal floating panel** (a find bar over a live terminal), which is `opaque: false` on something that is not a popup.

## When a sub-view is its own layer

Because routing and focus follow stack position, a sub-view that **covers** the surface beneath it and **owns its keys** must be pushed as its own layer — otherwise the host keeps routing keys to the now-hidden view (a within-surface leak).

- Use a **`modalLayer`** when the surface below stays as visible context (the hub's "New Task" form, Add-Repo).
- Use a **`pageLayer`** when it is a full replacement of its stack's region (the hub's keyboard cheatsheet).
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

Passive elements on `document.body` above everything; they take no focus and never participate in routing.

### Preview panes

A *horizontal split*, not a stack — the terminal stays live and focused beside them.
Modelling a split as a stack would be a category error.

### Crit panel's `<iframe>`

Once the user clicks *into* it, keydown events fire in the iframe's cross-origin document and never reach the host window, so routing (and every global shortcut) is inert until focus returns to host chrome.
This is a browser constraint the layer system cannot change, and per-tab stacks do not affect it; the review takes focus on activation so `q`/Esc works until the user deliberately clicks inside the iframe.

## Where things live

| Knowledge | Canonical home |
|---|---|
| The `LayerSpec` contract, the behavioral flags and their defaults, dismiss decisions, `RouteResult` | `types.ts` |
| Stack ordering, mounting, focus lifecycle (push/pop/reveal/replaceTop), the key walk | `LayerStack.ts` |
| The single keydown listener and the three-stage routing order | `LayerRouter.ts` |
| The four authoring presets and why each flag is set | `presets.ts` |
| Window-stack singleton accessor | `stack.ts` |
| Tab focus trap | `tabTrap.ts` |
| App-wide shortcut policy and each shortcut's precondition | `../KeyboardShortcuts.ts` |
| A tab's own stack and its terminal layer | `../../session/SessionTab.ts` |
| How a modal handles Escape/Enter, backdrop routing, the Tab trap | `../components/ModalDialog.ts` |
| Why the system exists, and the alternatives rejected | [ADR-003](../../../../docs/adrs/ADR-003-ember-layer-system.md) |
