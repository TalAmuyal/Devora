# Layer System (Sapphire) — current reference

Sapphire's port of ADR-003: stacked surfaces are routed and focused by their position in a stack, and their extent comes from which stack they are in.
The contract is Ember's ([`project-ember/src/ui/layers/CLAUDE.md`](../../../project-ember/src/ui/layers/CLAUDE.md)); this file records how Sapphire implements it and where it differs.

## Topology: n + 1 stacks

- **The window stack** (`LayerRouter.windowStack`), drawn by a `LayerHost` overlaid on the whole window (the tab bar included): hubs, the Command Palette, the User Guide, dialogs.
- **One stack per tab** (`Surfaces.tabStacks` in `AppShell`), made when the tab enters the core's state and cleared when it leaves; its bottom layer is the tab's terminal, pushed as soon as the tab has one, before it runs, so that it takes the keys typed into a new tab.
	A tab creating its workspace has its task-creation progress page first, a page pushed when the tab enters the state, and no terminal until the workspace exists; the terminal then goes in under the page (`LayerStack.insertAtBottom`, which takes no focus under another layer), and the page is removed, revealing the terminal, which takes the focus if its tab is active.

A surface covers the window iff it is in the window stack; the presets declare behavior, never extent.

## Mechanism, wiring and policy

- `LayerStack` is the mechanism: order, focus, dismissal and what a key walk reports; it knows no surface and has no key monitor.
- `LayerRouter` is the wiring: the one key-down entry point (`handle`), installed as a local `NSEvent` monitor for the window, so it runs before AppKit dispatches the key to the menu, the first responder or libghostty.
- `AppShell/AppShortcuts.swift` is the policy: the app-wide shortcuts and each one's availability.
- `AppShell/Surfaces.swift` is the composition root: it makes the router, keeps the tab stacks in step with the state, and puts each terminal at the bottom of its tab's stack.

## Key dispatch

`LayerRouter.handle` runs three stages: the app-wide shortcuts, the window stack top-down, then the active tab's stack (only if the window stack passed the key).
A stack walk reports `consumed` (the monitor drops the event), `blocked` (the event goes on to the first responder, which is how the terminal layer gets typing) or `passed`.
A top layer that traps Tab (`trapsTab`, set by `modalLayer`) gets Tab and Shift+Tab before the walk, through its `cycleFocus`, and the walk consumes them, so AppKit's key-view loop never moves the focus to a surface beneath.
Keys nothing consumes reach libghostty as AppKit delivers them, so typing, Ctrl+C, Option combos, dead keys and the IME are untouched; Command chords are never shortcuts, so the menu's (Cmd+Q, Cmd+W) keep working.

Shortcuts match on the physical key (`KeyPress.keyCode`, a `kVK_*` code), since Control turns a key's characters into control codes and Shift changes them; dismissal matches `q` on the characters (layout-dependent, as Ember's `e.key`) and Escape on the key code.
A focused text field or text view (the window's first responder is an `NSTextView`, which a field editor is) keeps `q` and Escape, as Ember's editable-element guard does.

## Shortcut availability

| Shortcut | Available |
|---|---|
| Font size: `Ctrl+1/2/3` (12/15/26), `Ctrl+=` (15), `Ctrl+Shift+=`/`-` (±2, within 8–40) | Always |
| `Ctrl+S` (open the Workspace Hub; never closes it), `Ctrl+Shift+S` (new shell) | When no opaque layer is in the window stack |
| `Ctrl+←/→` (switch tab, wrapping), `Ctrl+Shift+←/→` (move the active tab, not wrapping) | When no opaque layer is in the window stack |

An unavailable shortcut is still consumed, so it reaches neither the surface nor a terminal.
The font size is one size for every terminal (`TerminalRuntime.setFontSize`), kept in memory as Ember keeps it; unlike Ember's, it scales only the terminals, not the rest of the UI.
Option and Command chords are never shortcuts, where Ember's font-size keys ignore them.

## How later surfaces plug in

- A hub, the guide or the palette: push `pageLayer(LayerSpec(...))` on `windowStack`, with `focus` set to what should take the keyboard; with no `focus`, the window's first responder is cleared, so keys reach no view.
	The Workspace Hub (`Surfaces.openWorkspaceHub`) is the first: it has no `focus`, handles its keys in `onKey`, and consumes the plain keys it doesn't use, which would otherwise reach no view and beep.
- A dialog: `modalLayer`, pushed on the window stack, which traps Tab; as in Ember, the dialog owns Escape and Enter in its `onKey` and leaves `q` to the stack's dismissal.
	The confirmation dialog (`AppShell/ConfirmationDialog.swift`, `Surfaces.confirm`) is the first: it has no `focus`, so the window's first responder is cleared, and draws the focus of its buttons itself, which `cycleFocus` moves.
	The New Task dialog (`AppShell/TaskCreation/NewTaskDialog.swift`) has text fields: its `focus` gives the title field the first responder, its `onKey` returns false for the keys typed into a field, which the opaque modal then blocks, so they reach the field and nothing beneath, and `cycleFocus` moves between its fields and its buttons, clearing the first responder on a button.
- A surface over one tab: a page pushed on that tab's stack, which shows only while its tab is active and leaves the tab shortcuts available, since only the window stack's opaque layers make them unavailable.
	The task-creation progress page (`AppShell/TaskCreation/CreationProgressPage.swift`) is the first: it has no `focus`, answers Escape and `q` in its `onUserDismissRequest`, and consumes the plain keys it doesn't use, which would otherwise reach whatever has the focus, such as another tab's terminal.
- A dropdown: `popupLayer`, which takes no focus and passes on the keys it doesn't claim; Escape and `q` dismiss it.
	As Ember's popups are mounted by their caller, the surface it belongs to draws it, and its layer's content is empty: the Workspace Hub's header dropdowns are the first (`Surfaces.toggleHubMenu`).
	Close it before removing the surface beneath: removing a layer that isn't the top reveals nothing, so the focus wouldn't move on.
- `F1` (User Guide) is a key-down: a new `AppShortcut` case, matched before any layer like `Ctrl+S` (Workspace Hub).
- Shift-Shift (Command Palette) arrives as `.flagsChanged` events, not key-downs: its tracking needs a monitor of its own in `AppShortcuts`, while every key-down already reaches `AppShortcuts.handle` first, which is where a pending tap is reset.

## Where things live

| Knowledge | Home |
|---|---|
| `LayerSpec`, the flags, `RouteResult`, `DismissDecision`, the presets | `Layer.swift` |
| Order (`push`, `insertAtBottom`, `remove`), focus, dismissal, the key walk | `LayerStack.swift` |
| The entry point, the three stages, the window's focus | `LayerRouter.swift` |
| `KeyPress`, `KeyModifiers`, the `kVK_*` codes in use | `KeyPress.swift` |
| Draw order (stack order, bottom to top, in a `ZStack`) | `LayerHost.swift` |
| The shortcuts and their availability | `../AppShell/AppShortcuts.swift` |
| The tab stacks, the terminal layer and the task-creation progress page | `../AppShell/Surfaces.swift` |
