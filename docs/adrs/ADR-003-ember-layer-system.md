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
Every stacked surface becomes a *layer*; nothing else registers a global key listener.
Routing walks the stack from the top down, and focus is resolved at each transition from stack state — never from a value captured earlier.

### Layer kinds

A layer declares one of four kinds, which fixes its stacking and modality behavior:

- **`page`** — a full-screen surface (the hubs, User Guide, Command Palette). Opaque modality barrier.
- **`modal`** — a dialog over a backdrop. Opaque barrier; gets a Tab focus trap.
- **`popup`** — an anchored, light-dismiss surface (dropdown menus, future popovers). Stays where the caller placed it in the DOM, never moves focus, and is **transparent** to keys it does not handle.
- **`panel`** — a per-session region overlaying the terminal (Crit, task-creation). Inserted at the **bottom** of the stack (it represents session content and must never intercept a page/modal above it) and **transparent** to unhandled keys.

`popup` and `panel` transparency is what makes non-modal surfaces work: unhandled keys fall through to lower layers and ultimately to the base global shortcuts.

Kind is a classification of *behavior* — modality, focus ownership, and key routing — not of visual size. The Command Palette is a `page` even though it renders as a floating box over a transparent backdrop: it owns keyboard focus, is mutually exclusive with the other pages, and is opaque to keys (the terminal below it is inert). A `popup`, by contrast, never takes focus and is transparent to keys it does not handle. The palette's see-through backdrop is cosmetic; its wrapper still covers the window and its semantics are page semantics.

### Layer contract

A layer is pushed with a spec (`src/ui/layers/types.ts`):

- `onKey(e) => boolean` — layer-specific keys, tried **first**; `true` consumes.
- `onUserDismissRequest() => 'close' | 'handled' | 'veto'` — the response to a dismissal request (`q`, `Escape`, and `Ctrl+S` for pages). `close` pops the layer; `handled` means the layer did something internal instead (closed the hub cheatsheet, replaced itself) and the key is consumed without popping; `veto` refuses (the zero-profile lock) and still consumes.
- `onCleanup()` — runs exactly once on pop/remove/replace.
- `onReveal()` — runs when the layer becomes top again after a covering **page** pops (refresh-on-reveal for returning from another page). A transient `modal`/`popup`/`panel` pop restores focus without re-running it.
- `resolveFocus() => Focusable | null` — the focus target, resolved at push and at reveal.

The stack supports both **true stacking** — the lower page stays alive and is revealed with its state preserved — and **replace** semantics, an atomic swap of the top layer; the call site chooses which a given transition uses. Removing a layer that is not on top does no focus or reveal work, and removing a page or modal also removes any popup anchored inside it.

### Key-dispatch algorithm

`consume(e)` is `preventDefault()` + `stopImmediatePropagation()`.
`stopImmediatePropagation` (not `stopPropagation`) guarantees that no other window-capture listener can act on a consumed key; sibling window listeners survive `stopPropagation`, which is the root cause of the cheatsheet double-fire noted above.

```
handleKeydown(e):
  keyObserver?.(e)                                   # Shift-Shift tracking; never consumes
  if stack.isEmpty():
    if globalHandler(e): consume(e)
    return                                           # unconsumed keys flow to the focused terminal
  if e.key == 'Tab' and top().kind == 'modal':
    cycleFocus(top().wrapper, e.shiftKey); consume(e); return
  for layer in stack, top first:
    if layer.onKey?.(e) == true: consume(e); return
    if isDismissKey(e, layer):
      if editableFocusedWithin(layer): return        # UNCONSUMED: 'q' types; Escape reaches the input's own handler
      decision = layer.onUserDismissRequest?.() ?? 'close'
      if decision == 'close': pop(layer)
      consume(e); return                             # 'handled' and 'veto' consume too
    if layer.kind in ('page', 'modal'):
      if allowedThroughBarrier(e, layer.kind) and globalHandler(e): consume(e)
      return                                         # the barrier ends the walk
    continue                                         # 'popup' and 'panel' pass unhandled keys down
  if globalHandler(e): consume(e)                    # only panels/popups were present

isDismissKey(e, layer):
  (Escape or q, unmodified)                                  -> true
  (Ctrl+S, no Shift, code 'KeyS') and layer.kind == 'page'   -> true
  else                                                       -> false

editableFocusedWithin(layer):
  isEditableElementFocused() and layer.wrapper.contains(document.activeElement)
```

Global shortcuts (`Ctrl+S`, `F1`, `Ctrl+Shift+S`, `Ctrl+←/→`, `Ctrl+Shift+←/→`, font sizing) sit at the **base** of this dispatcher, consulted only when a layer lets a key through its barrier.
Shift-Shift (open Command Palette) fires on keyup, outside the dispatcher; a non-consuming `keyObserver` sees every keydown to track the lone-Shift state.

The Command Palette's "type-first" behavior needs no special case: its search field is focused on open, so `q` hits `editableFocusedWithin` and types, while Escape passes through to the field's own keydown handler, which blurs and requests close.

### Modality-barrier matrix

`allowedThroughBarrier` is one explicit function.
Under a `page`, only `F1` and the font-size combos reach the base; under a `modal`, nothing does.
`popup`/`panel` are transparent.

| Shortcut | base (empty) | under panel/popup only | under page | under modal |
|---|---|---|---|---|
| `q` / Escape | → focused element | dismiss top layer | route via `onUserDismissRequest` | dismiss (cancel) via the modal's `onKey` |
| `Ctrl+S` (Workspace Hub) | open hub | open hub | **dismiss key** (hub: close; Settings: back-to-hub; palette: no-op via its `onKey`) | blocked |
| `F1` (User Guide) | open guide | open guide | **allowed** — pushes the guide over the page | blocked |
| `Ctrl+Shift+S` (new session) | allowed | allowed | **blocked** | blocked |
| `Ctrl+←/→` (switch tab) | allowed | **allowed** | **blocked** | blocked |
| `Ctrl+Shift+←/→` (move tab) | allowed | allowed | blocked | blocked |
| Font size (`Ctrl+1/2/3`, `Ctrl±`) | allowed | allowed | **allowed** | blocked |
| Shift-Shift (palette) | allowed | allowed | blocked (guard: no page/modal on stack) | blocked |

Blocking `Ctrl+Shift+S` and `Ctrl+←/→` under a page is deliberate: both silently acted beneath an open hub and stole focus.

### Focus lifecycle

- **push** — mount the wrapper (`page` → `#app`; `modal` → `document.body` with a `${name}-backdrop` class; `popup`/`panel` stay where the caller placed them), set `tabIndex = -1`, and focus `resolveFocus() ?? wrapper` — except `popup`, which never moves focus.
- **pop / remove-of-top** — run `onCleanup` (guarded by try/catch), remove the wrapper, then focus the revealed layer's `resolveFocus() ?? wrapper`; its `onReveal` runs only when the removed cover was a `page` (a navigation refresh — a transient modal does not re-run it, since on confirm the caller refreshes and on cancel nothing changed). When the stack empties, focus `resolveBaseFocus()` (the active session's terminal pane), **resolved at that moment** — this replaces every captured `restoreFocusTo` and fixes the stale-target defect.
- **remove-of-non-top** — cleanup and detach only, with no focus or reveal work, so a backend-initiated close of a hidden session's panel does not steal focus.
- **replaceTop** — old layer's cleanup, new layer mounted and focused once, no reveal of anything below.
- **Tab trap** — only for a `modal` on top. `src/ui/layers/tabTrap.ts` collects focusables within the wrapper and cycles, recovering if focus escaped.

Because routing keys off stack position, the editable-focus guard now matters only when the focused editable element is *inside the top layer's wrapper*.
The terminal's xterm textarea never is, so overlays and panels dismiss correctly regardless of terminal focus.
Panels additionally take focus when their tab is visible, so keystrokes stop feeding an invisible terminal.

### Two non-obvious decisions

- **Modals own Escape and Enter in their `onKey`, not the central dismissal convention.** The text-input, add-repo, and clone-repo dialogs focus a plain `<input>` that has no element-level Escape handler, so routing Escape through the `editableFocusedWithin → pass through` path would make it a dead key. The modal primitive therefore handles Escape and Enter itself (its `onKey` runs before the editable guard) and leaves `q` to the guard — `q` types when an input is focused and cancels when a button is. Pages keep the opposite behavior: Escape passes through so a focused search field can blur. This asymmetry preserves each surface's current single-press behavior.
- **The acceptance harness drives the same entry points as production.** Tests open and dismiss surfaces through the same code paths the app uses, rather than through a compatibility adapter that could let the tests exercise different wiring than users do.

### What stays outside the layer system

Toasts and error banners remain passive elements on `document.body` above everything; they take no keyboard focus and never participate in routing.
The Crit panel embeds an `<iframe>`; once the user clicks *into* it, keydown events fire in the iframe's document and never reach the host window, so layer routing (and every global shortcut) is inert until focus returns to host chrome.
This is a browser constraint the layer system cannot change; the panel taking focus on activation means q/Esc works until the user deliberately clicks inside the iframe.

## Consequences

- Key routing and focus become a function of stack position, not of registration order or incidental focus. The seven cross-layer defects above are fixed structurally rather than patched per surface.
- New pop-up kinds are added by pushing a layer of the right kind; the four kinds already cover modal forms, anchored popovers, non-modal panels, and full pages, so the common cases need no new routing code.
- Some navigation becomes true stacking and changes visibly (intended): `F1` and Health/Settings opened from the hub now stack over it and reveal it on dismissal instead of replacing it; `Ctrl+Shift+S` and `Ctrl+←/→` are inert while a page is open; `q` now cancels modals and closes dropdowns (completing the vim-style dismissal convention). These are pinned by new acceptance scenarios.
- `OverlayManager`, the per-hub and per-dialog key listeners, the `DropdownMenu` key listener, the `commandPaletteOpen` flag, and two `main.ts` monkey-patches are removed. The single dispatcher plus per-layer specs replace them.
- Focus behavior inside an iframe-backed panel remains bounded by the browser; this is documented rather than worked around.
