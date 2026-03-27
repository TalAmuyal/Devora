# TUI Navigation Key Binding Spec

Package: `internal/tui`

For page model definitions and the state machine, see [tui.md](tui.md). For component details (TextInputModel, PathPickerModel, etc.), see [tui-components.md](tui-components.md).

## Overview

Three keys govern navigation and exit: `ctrl+c` (quit app), `esc` (context-sensitive back), and `q` (back in navigation mode only). `ctrl+d` is not used.

Pages operate in one of two modes: **navigation mode** (page-level key handling) and **insert mode** (text input captures keystrokes). On pages with text inputs, `esc` first transitions from insert mode to navigation mode; a second `esc` performs the "back" action. On pages without text inputs, there is no insert mode, so `esc` performs "back" immediately.

## Key Roles

| Key | Role |
|---|---|
| `ctrl+c` | **Quit app.** Always, everywhere, unconditionally. |
| `esc` | **Unfocus or back.** If in insert mode: switch to navigation mode. If in navigation mode: go back. At root: quit app. |
| `q` | **Back (navigation only).** Same as the "back" action of `esc`, but only fires in navigation mode. In insert mode, `q` is a literal character typed into the input. |
| `ctrl+d` | **Not used.** No-op on every page, every mode. |

"Back" means: return to the previous page. At root (WorkspaceList) or in standalone flows (AddRepo, first-run ProfileRegistration with `hasBack=false`), "back" means quit the app.

## Navigation Mode vs Insert Mode

### Definitions

- **Insert mode**: A text input component has focus and is capturing keystrokes. Characters typed go into the input. Only `ctrl+c` and `esc` are handled at the page level; all other keys are consumed by the text input.
- **Navigation mode**: No text input has focus. Keys like `q`, `esc`, and shortcut keys are handled at the page level.

### Two-Stage Esc

On pages that contain text inputs:
1. Page opens with the first text input focused (insert mode).
2. First `esc`: unfocus the text input, enter navigation mode. The cursor/highlight leaves the text input.
3. Second `esc` (now in navigation mode): perform the "back" action.

On pages without text inputs, there is no insert mode. `esc` performs "back" immediately (single stage).

### Visual Indicator

When in navigation mode on a form page, the previously-focused text input must visually lose its active/focused styling (e.g., border color change, cursor hidden). No input should have the focused style.

Re-entering insert mode: pressing `enter`, `tab`, or any page-specific focus-cycling key on a text input field moves focus back to that input and re-enters insert mode.

## Behavior Matrix

### Legend

| Symbol | Meaning |
|---|---|
| QUIT | Quit the entire application (`tea.Quit`) |
| BACK | Return to previous page (`showWorkspaceListMsg`), or quit if at root/standalone |
| UNFOCUS | Leave insert mode, enter navigation mode on the current page |
| NO-OP | Key is ignored |
| INPUT | Character is captured by the text input |

### WorkspaceList (root)

No text inputs. Always in navigation mode.

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | QUIT |
| `q` | QUIT |

### NewTask

Focus states: `taskName` (text input), `repos` (checkbox list), `submit` (button).

| Key | taskName (insert) | repos (nav) | submit (nav) |
|---|---|---|---|
| `ctrl+c` | QUIT | QUIT | QUIT |
| `esc` | UNFOCUS | BACK | BACK |
| `q` | INPUT | BACK | BACK |

### Creation

Two sub-states: in-progress and error.

**In-progress** (all input blocked except panic button):

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | NO-OP |
| `q` | NO-OP |

**Error state** (no text inputs, navigation mode):

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | BACK |
| `q` | BACK |

### DeleteConfirm

No text inputs. Always in navigation mode.

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | BACK (cancel delete) |
| `q` | BACK (cancel delete) |

### RegisterRepo

Single text input page. Starts in insert mode.

| Key | Insert mode | Navigation mode |
|---|---|---|
| `ctrl+c` | QUIT | QUIT |
| `esc` | UNFOCUS | BACK |
| `q` | INPUT | BACK |

### Settings

Three focusable fields navigated via `j`/`k`. Three modes: navigation, editing (prepare command text input), and confirm-delete (y/n prompt). Starts in navigation mode.

**Navigation mode** (no text input focused):

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | BACK |
| `q` | BACK |

**Editing mode** (prepare command text input focused):

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | Cancel edit, return to navigation mode (restores previous value) |
| `q` | INPUT |

**Confirm-delete mode** (y/n prompt shown):

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | Cancel, return to navigation mode |
| `y` | Confirm delete |
| `n` | Cancel, return to navigation mode |

### ProfileRegistration

Focus states: `profilePath` (PathPicker), `profileName` (text input), `submit` (button).

PathPicker has two internal modes: **type mode** (insert mode) and **browse mode** (navigation mode).

| Key | profilePath type (insert) | profilePath browse (nav) | profileName (insert) | submit (nav) |
|---|---|---|---|---|
| `ctrl+c` | QUIT | QUIT | QUIT | QUIT |
| `esc` | UNFOCUS | BACK | UNFOCUS | BACK |
| `q` | INPUT | BACK | INPUT | BACK |

When `hasBack=false` (first-run): BACK becomes QUIT.

### AddRepo (standalone)

Single text input for postfix. "Back" always means quit (standalone flow, no previous page).

**Form state:**

| Key | Text focused (insert) | Non-text focused (nav) |
|---|---|---|
| `ctrl+c` | QUIT | QUIT |
| `esc` | UNFOCUS | QUIT |
| `q` | INPUT | QUIT |

**In-progress** (all input blocked except panic button):

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | NO-OP |
| `q` | NO-OP |

**Error state:**

| Key | Behavior |
|---|---|
| `ctrl+c` | QUIT |
| `esc` | QUIT |
| `q` | QUIT |

## Footer Hints

Footer hints reflect the current mode. When mode changes, hints update.

| Page | Navigation Mode | Insert Mode |
|---|---|---|
| WorkspaceList | `q quit` | N/A |
| NewTask | `esc/q back` | `esc unfocus` |
| Creation (in-progress) | _(no hints)_ | N/A |
| Creation (error) | `esc/q back` | N/A |
| DeleteConfirm | `esc/q cancel` | N/A |
| RegisterRepo | `esc/q back` | `esc unfocus` |
| Settings (navigation) | `enter select`, `j/k navigate`, `esc/q back` | N/A |
| Settings (editing) | N/A | `enter save`, `esc cancel` |
| Settings (confirm-delete) | `y confirm`, `n/esc cancel` | N/A |
| ProfileRegistration | `esc/q back` | `esc unfocus` |
| ProfileRegistration (first-run) | `esc/q quit` | `esc unfocus` |
| AddRepo (form) | `esc/q quit` | `esc unfocus` |
| AddRepo (in-progress) | _(no hints)_ | N/A |
| AddRepo (error) | `esc/q quit` | N/A |

`ctrl+c` is intentionally not shown in footers. It always works; power users know it.

Page-specific action hints (e.g., `enter select`, `space toggle`, `tab next field`) are shown alongside the navigation hints above.

## Key Handling Pipeline

Key events are processed in this order:

1. **`ctrl+c`**: Handled at `AppModel.Update()` (or `AddRepoModel.Update()` for standalone). Returns `tea.Quit` immediately. No child component sees this key.
2. **In-progress guard**: If the current page is in an in-progress state (Creation, AddRepo submission), swallow all keys. Return early.
3. **`esc` in insert mode**: If the active page has a focused text input, call the page-level handler to unfocus it. Do not propagate to the text input component.
4. **Navigation mode dispatch**: Page handles `esc` (back), `q` (back), and all other navigation keys directly.

### Component Responsibilities

- **AppModel** / **AddRepoModel**: Handle `ctrl+c`. Route all other keys to the active page.
- **Page models**: Own focus state. Determine insert vs navigation mode. Handle `esc` (unfocus or back) and `q` (back in navigation mode). Emit navigation messages (`showWorkspaceListMsg`, `tea.Quit`) to the parent.
- **Text input components** (TextInputModel, PathPickerModel in type mode): Receive keys only when focused. Never see `esc` (page intercepts it). Never see `ctrl+c` (root model intercepts it).
- **PathPickerModel**: Exposes `Mode()` so the parent page can distinguish type mode (insert) from browse mode (navigation).
