# Layer System (Sapphire) — current reference

Sapphire's port of ADR-003: stacked surfaces are routed and focused by their position in a stack, and their extent comes from which stack they are in.
The contract is Ember's ([`project-ember/src/ui/layers/CLAUDE.md`](../../../project-ember/src/ui/layers/CLAUDE.md)); this file records how Sapphire implements it and where it differs.

## Topology: n + 1 stacks

- **The window stack** (`LayerRouter.windowStack`), drawn by a `LayerHost` overlaid on the whole window (sidebar included): hubs, the Command Palette, the User Guide, dialogs.
- **One stack per tab** (`Surfaces.tabStacks` in `AppShell`), made when the tab enters the core's state and cleared when it leaves; its bottom layer is the tab's terminal, pushed once the terminal runs, so a tab's stack is empty until then.

A surface covers the window iff it is in the window stack; the presets declare behavior, never extent.

## Mechanism, wiring and policy

- `LayerStack` is the mechanism: order, focus, dismissal and what a key walk reports; it knows no surface and has no key monitor.
- `LayerRouter` is the wiring: the one key-down entry point (`handle`), installed as a local `NSEvent` monitor for the window, so it runs before AppKit dispatches the key to the menu, the first responder or libghostty.
- `AppShell/AppShortcuts.swift` is the policy: the app-wide shortcuts and each one's availability.
- `AppShell/Surfaces.swift` is the composition root: it makes the router, keeps the tab stacks in step with the state, and puts each terminal at the bottom of its tab's stack.

## Key dispatch

`LayerRouter.handle` runs three stages: the app-wide shortcuts, the window stack top-down, then the active tab's stack (only if the window stack passed the key).
A stack walk reports `consumed` (the monitor drops the event), `blocked` (the event goes on to the first responder, which is how the terminal layer gets typing) or `passed`.
Keys nothing consumes reach libghostty as AppKit delivers them, so typing, Ctrl+C, Option combos, dead keys and the IME are untouched; Command chords are never shortcuts, so the menu's (Cmd+Q, Cmd+W) keep working.

Shortcuts match on the physical key (`KeyPress.keyCode`, a `kVK_*` code), since Control turns a key's characters into control codes and Shift changes them; dismissal matches `q` on the characters (layout-dependent, as Ember's `e.key`) and Escape on the key code.
A focused text field or text view (the window's first responder is an `NSTextView`, which a field editor is) keeps `q` and Escape, as Ember's editable-element guard does.

## Shortcut availability

| Shortcut | Available |
|---|---|
| Font size: `Ctrl+1/2/3` (12/15/26), `Ctrl+=` (15), `Ctrl+Shift+=`/`-` (±2, within 8–40) | Always |
| `Ctrl+Shift+S` (new shell) | When no opaque layer is in the window stack |
| `Ctrl+←/→` (switch tab, wrapping), `Ctrl+Shift+←/→` (move the active tab, not wrapping) | When no opaque layer is in the window stack |

An unavailable shortcut is still consumed, so it reaches neither the surface nor a terminal.
The font size is one size for every terminal (`TerminalRuntime.setFontSize`), kept in memory as Ember keeps it; unlike Ember's, it scales only the terminals, not the rest of the UI.
Option and Command chords are never shortcuts, where Ember's font-size keys ignore them.

## How later surfaces plug in

- A hub, the guide or the palette: push `pageLayer(LayerSpec(...))` on `windowStack`, with `focus` set to what should take the keyboard; with no `focus`, the window's first responder is cleared, so keys reach no view.
- A dialog: `modalLayer`, pushed on the window stack; Ember's modal also traps Tab within the dialog, which comes with the first dialog.
- A dropdown: `popupLayer`, which takes no focus and passes on the keys it doesn't claim.
- `Ctrl+S` (Workspace Hub) and `F1` (User Guide) are key-downs: new `AppShortcut` cases, matched before any layer like the others.
- Shift-Shift (Command Palette) arrives as `.flagsChanged` events, not key-downs: its tracking needs a monitor of its own in `AppShortcuts`, while every key-down already reaches `AppShortcuts.handle` first, which is where a pending tap is reset.

## Where things live

| Knowledge | Home |
|---|---|
| `LayerSpec`, the flags, `RouteResult`, `DismissDecision`, the presets | `Layer.swift` |
| Order, focus, dismissal, the key walk | `LayerStack.swift` |
| The entry point, the three stages, the window's focus | `LayerRouter.swift` |
| `KeyPress`, `KeyModifiers`, the `kVK_*` codes in use | `KeyPress.swift` |
| Draw order (stack order, bottom to top, in a `ZStack`) | `LayerHost.swift` |
| The shortcuts and their availability | `../AppShell/AppShortcuts.swift` |
| The tab stacks and the terminal layer | `../AppShell/Surfaces.swift` |
