# TUI Package Spec

Package: `internal/tui`

## Purpose

The TUI is the primary user interface. It provides workspace management (listing, creating, deleting), profile registration, repo registration, and settings editing. Built with Bubble Tea (Elm architecture) and Lip Gloss for styling.

The visual design uses a panel-based dashboard with cards, diamond selection indicators, and category badges.

For theme loading, palette structure, and style definitions, see [tui-theme.md](tui-theme.md).

For workspace data gathering, creation flow, deletion rules, and add-repo logic, see [tui-operations.md](tui-operations.md).

## Entry Points

### DefaultThemePath

```go
func DefaultThemePath() string
```

Returns the path to the Kitty current-theme config file (`~/.config/kitty/current-theme.conf`). Used by the CLI to obtain the `themePath` parameter for `RunWorkspaceUI` and `RunAddRepo`.

### RunWorkspaceUI

```go
func RunWorkspaceUI(themePath string, repoNames []string, showProfileRegistration bool) (*AppResult, error)
```

Main entry point. Creates a `tea.Program` with `AppModel`, runs it, and returns the result.

- `themePath`: path to a Kitty theme file for palette extraction (empty string uses defaults)
- `repoNames`: registered repo names for the active profile
- `showProfileRegistration`: if true, starts on the profile registration page (first-run flow)

Returns `*AppResult` with either `SelectedWorkspace` (user picked an existing workspace) or `NewWorkspace` (user created a new workspace). Returns `nil` if the user quit without selection.

### NewAppModel

```go
func NewAppModel(palette ThemePalette, repoNames []string, showProfileRegistration bool) AppModel
```

Constructs the main app model. Initializes all page models, loads the active profile name, and sets the starting page (`PageProfileRegistration` if `showProfileRegistration` is true, `PageWorkspaceList` otherwise). Called by `RunWorkspaceUI`.

### AppModel.Result

```go
func (m AppModel) Result() *AppResult
```

Returns the app result after the TUI exits. Used by `RunWorkspaceUI` to extract the result from the final `tea.Model`.

### RunAddRepo

```go
func RunAddRepo(themePath string, workspacePath string, repoNames []string) error
```

Standalone `tea.Program` for adding a repo to an existing workspace. Separate from the main app. For details on the add-repo flow, see [tui-operations.md#add-repo](tui-operations.md#add-repo).

### NewAddRepoModel

```go
func NewAddRepoModel(palette ThemePalette, workspacePath string, repoNames []string) AddRepoModel
```

Constructs the standalone add-repo model. Called by `RunAddRepo`.

### CreateAndAttachSession

```go
func CreateAndAttachSession(wsPath, sessionName string) error
```

Post-TUI helper called by the CLI after the TUI exits with a result. Creates a Kitty backend, reads config for default app and timeout, then delegates to `terminal.CreateAndAttach`. Lives in the `tui` package but is not a TUI component -- it bridges TUI output to terminal session creation. For implementation details, see [tui-operations.md#session-creation](tui-operations.md#session-creation).

## Page State Machine

The app uses a `Page` enum with switch-based dispatch (no polymorphic interface).

```
Page = PageWorkspaceList | PageNewTask | PageCreation | PageDeleteConfirm
     | PageRegisterRepo | PageSettings | PageProfileRegistration
```

### Transitions

```
PageProfileRegistration  (first-run: entry point; normal: via P key)
        |
        | profileActivatedMsg
        v
PageWorkspaceList <--- showWorkspaceListMsg --- (all sub-pages)
   |    |    |    |
   |    |    |    +--- showSettingsMsg ---------> PageSettings
   |    |    +--- showRegisterRepoMsg ----------> PageRegisterRepo
   |    +--- showNewTaskMsg --------------------> PageNewTask
   +--- requestDeleteMsg -----------------------> PageDeleteConfirm
   +--- deactivateCompleteMsg ------------------> (refresh + notification)
   +--- showProfileRegistrationMsg <-----------> PageProfileRegistration
   +--- WorkspaceInfo (selected) ---------------> tea.Quit (with result)

PageNewTask --- NewTaskResult ---> PageCreation --- creationDoneMsg ---> tea.Quit (with result)
                                                --- creationErrorMsg --> (stays, shows error)
                                                --- q/escape (error) --> PageWorkspaceList

PageDeleteConfirm --- deleteCompleteMsg ---> PageWorkspaceList (+ refresh)
                  --- deleteErrorMsg ------> (stays, shows error)
```

### Global Keys (AppModel)

These bindings apply to pages within `AppModel` only. The standalone `AddRepoModel` has its own key bindings (see [Standalone Models](#addrepomodel)).

| Key | Page | Behavior |
|-----|------|----------|
| `ctrl+d` | `PageWorkspaceList` | Quit the app |
| `ctrl+d` | Any other page | Cancel and return to workspace list |

### Hub Model

`PageWorkspaceList` is the hub. All sub-pages return to it via `showWorkspaceListMsg`. Form pages use `ctrl+c` as the back key (except first-run profile registration, where `ctrl+c` quits the app); the creation error state uses `q`/`escape`; the delete confirmation uses `n`/`escape`.

## AppModel

```go
type AppModel struct {
    activePage Page
    result     *AppResult

    // Embedded page models (concrete structs, not interfaces)
    workspaceList WorkspaceListModel
    newTask       NewTaskModel
    creation      CreationModel
    deleteConfirm DeleteConfirmModel
    registerRepo  RegisterRepoModel
    settings      SettingsModel
    profileReg    ProfileRegModel

    // Shared state
    styles      Styles
    palette     ThemePalette
    repoNames   []string
    profileName string

    // Creation progress channel
    creationCh chan tea.Msg

    // Notification system
    notifyText    string
    notifyIsError bool
    notifySeq     int

    width, height int
}
```

### Update dispatch

`AppModel.Update` handles:
1. `tea.WindowSizeMsg` -- propagates to all page models via `SetSize`
2. Page transition messages -- sets `activePage` and initializes the target page
3. Data/result messages -- handles workspace loading, creation progress, deletion, profile activation
4. `spinnerTickMsg` -- advances creation spinner frame (only while on creation page with no error)
5. `notifyMsg` / `notifyClearMsg` -- ephemeral 4-second notification system
6. `tea.KeyPressMsg` -- checks global bindings (`ctrl+d`), then delegates to the active page's `Update` method

### View layout

```
+----------------------------------------------+
|  DEVORA  .  Page Title            profile     |  <- header (2 lines, with separator)
|  > Active    Inactive    All                  |  <- tab bar (workspace list only)
|                                               |
|           Page Content                        |  <- content (variable height)
|                                               |
| check Notification text                       |  <- notification (0-1 lines)
|----------------------------------------------|  <- footer separator
|   key  Desc    key  Desc    key  Desc         |  <- footer bindings
+----------------------------------------------+
```

## Message Types

### Page transitions

- `showWorkspaceListMsg` -- navigate to workspace list
- `refreshWorkspacesMsg` -- reload workspace data
- `showNewTaskMsg` -- navigate to new task form
- `showSettingsMsg` -- navigate to settings page
- `showRegisterRepoMsg` -- navigate to register repo page
- `showProfileRegistrationMsg` -- navigate to profile registration page

### Data types

- `WorkspaceInfo` -- workspace metadata (path, name, category, task title, repos, git statuses, invalid reason)
- `NewTaskResult` -- submitted new task form (repo names, task name)
- `WorkspaceReadyResult` -- created workspace ready for session attachment (workspace path, session name)

### Async results

- `workspacesLoadedMsg{workspaces}` -- background workspace scan complete
- `workspaceLoadErrorMsg{err}` -- workspace scan failed (e.g., no profile configured, unreadable directory)
- `creationStatusMsg{text}` -- creation progress status text
- `repoAddedMsg{name}` -- repo added to workspace, preparing
- `repoReadyMsg{name}` -- repo worktree ready
- `creationDoneMsg{workspacePath, sessionName}` -- creation success
- `creationErrorMsg{err}` -- creation failure
- `requestDeleteMsg{info}` -- delete request (from workspace list, after blocker checks)
- `deleteCompleteMsg` -- deletion success
- `deleteErrorMsg{err}` -- deletion failure
- `deactivateCompleteMsg` -- deactivation success (triggers refresh + notification)
- `deactivateErrorMsg{err}` -- deactivation failure
- `notifyMsg{text, isError}` -- show ephemeral notification
- `notifyClearMsg{seq}` -- clear notification by sequence number
- `spinnerTickMsg` -- advance creation spinner frame
- `profileActivatedMsg` -- profile switch complete

## Page Models

Each page model is a concrete struct with these methods:
- `Update(msg tea.Msg) tea.Cmd` -- handle messages (primarily `tea.KeyPressMsg`), return commands
- `View() string` -- render content
- `borderTitle() string` -- page title for the header
- `ActionBindings() []components.KeyBinding` -- footer keybindings
- `SetSize(width, height int)` -- respond to terminal resize

Page models are NOT `tea.Model` -- they don't implement `Init()`. They are embedded in `AppModel` which owns the Bubble Tea lifecycle.

### Inline Error Clearing

All form pages that display inline validation errors (`errMsg`) clear the error when the user types into any field. The error persists across tab/shift+tab navigation and is only re-set on the next failed submit. This applies to: `NewTaskModel`, `RegisterRepoModel`, `ProfileRegModel`, and `AddRepoModel`.

### WorkspaceListModel

```go
func NewWorkspaceListModel(styles *Styles) WorkspaceListModel
```

Displays workspaces as styled cards with a diamond selection indicator. Each card shows task title (or workspace name), a category badge (ACTIVE/IDLE/INVALID), and a per-repo table with branch name and clean/dirty status.

**Key bindings:**
- `1`/`2`/`3` -- switch filter (Active, Inactive, All)
- `j`/`k`/`gg`/`G`/arrows -- vim navigation
- `enter` -- select workspace (exits TUI with result)
- `n` -- new task, `d` -- deactivate, `D` -- delete, `r` -- refresh
- `R` -- register repo, `P` -- add profile, `p` -- cycle profile
- `s` -- settings, `q`/`escape` -- quit

**Messages handled:** `tea.KeyPressMsg` (delegates navigation to embedded `ListModel`).

**Transitions:** Emits `showNewTaskMsg`, `showSettingsMsg`, `showRegisterRepoMsg`, `showProfileRegistrationMsg`, `refreshWorkspacesMsg`, `requestDeleteMsg`, `deactivateCompleteMsg`/`deactivateErrorMsg`, `WorkspaceInfo` (selection), or `tea.Quit`.

### NewTaskModel

```go
func NewNewTaskModel(repoNames []string, styles *Styles) NewTaskModel
```

Multi-field form with three tab stops: repo selection (checkbox list), task name (text input), and submit button. Validates that at least one repo is selected and the task name is non-empty.

**Key bindings:**
- `tab`/`shift+tab` -- cycle fields
- `space` -- toggle repo checkbox (when repos focused)
- `enter` -- submit (when on task name or submit field)
- `ctrl+c` -- back to workspace list

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `NewTaskResult` on submit, `showWorkspaceListMsg` on cancel.

### CreationModel

```go
func NewCreationModel(styles *Styles) CreationModel
```

Read-only progress display. Shows status text and per-repo progress (preparing/ready) with an animated Braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`, 80ms interval) on repos still preparing. On error, displays error message with option to go back.

The spinner is driven by `spinnerTickMsg`, handled by `AppModel.Update`: it advances the `spinnerFrame` index and schedules the next tick. The tick stops when creation completes or errors.

**Key bindings (error state only):**
- `q`/`escape` -- back to workspace list

**Messages handled:** `tea.KeyPressMsg` only. Creation progress messages (`creationStatusMsg`, `repoAddedMsg`, `repoReadyMsg`, `creationDoneMsg`, `creationErrorMsg`) and `spinnerTickMsg` are handled by `AppModel.Update`, which updates `CreationModel` fields directly.

**Transitions:** Emits `showWorkspaceListMsg` on error dismissal.

### DeleteConfirmModel

```go
func NewDeleteConfirmModel(styles *Styles) DeleteConfirmModel
```

Centered dialog showing workspace name, path, and a warning about the destructive operation.

**Key bindings:**
- `y` -- confirm deletion
- `n`/`escape` -- cancel, back to workspace list

**Messages handled:** `tea.KeyPressMsg` only. `deleteErrorMsg` is handled by `AppModel.Update`, which updates `DeleteConfirmModel.errMsg` directly.

**Transitions:** Emits `deleteCompleteMsg` or `deleteErrorMsg` after deletion attempt. Emits `showWorkspaceListMsg` on cancel. For delete blocking rules (active session, non-HEAD branch, dirty repo), see [tui-operations.md#delete-blocking-rules](tui-operations.md#delete-blocking-rules).

### RegisterRepoModel

```go
func NewRegisterRepoModel(styles *Styles) RegisterRepoModel
```

Single text input for entering a git repo path. The path is tilde-expanded (via `config.ExpandTilde`) before validation. Validates that the path exists, is a directory, and is a git repository before registering.

**Key bindings:**
- `enter` -- register the repo
- `ctrl+c` -- back to workspace list

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `notifyMsg` (success) + `showWorkspaceListMsg` on success, stays on page with error on failure.

### SettingsModel

```go
func NewSettingsModel(styles *Styles) SettingsModel
```

Single text input for the prepare command (shell command run after worktree creation). Loads current value on activation. Title includes profile name when available.

**Key bindings:**
- `enter` -- save settings
- `ctrl+c` -- back to workspace list

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** On successful save, emits `notifyMsg` (success) + `showWorkspaceListMsg` (auto-navigates back). On save error, emits `notifyMsg` (error) and stays on page. Emits `showWorkspaceListMsg` on cancel (`ctrl+c`).

### ProfileRegModel

```go
func NewProfileRegModel(styles *Styles, hasBack bool) ProfileRegModel
```

Two text inputs (path + name) and a submit button, with three tab stops. Behavior varies based on `hasBack`:
- First-run (`hasBack=false`): `ctrl+c` quits the app entirely
- Normal (`hasBack=true`): `ctrl+c` returns to workspace list

Validates that path is provided, and name is required for uninitialized profiles. On success, activates the profile.

**Key bindings:**
- `tab`/`shift+tab` -- cycle fields
- `enter` -- register (when on name or submit field)
- `ctrl+c` -- back or quit (depends on `hasBack`)

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `profileActivatedMsg` on success, `showWorkspaceListMsg` on cancel (when `hasBack` is true).

## Standalone Models

### AddRepoModel

`AddRepoModel` is a standalone `tea.Model` (implements `Init`, `Update`, `View`) run via `RunAddRepo` in its own `tea.Program`. It is NOT embedded in `AppModel`. For its form fields, collision checking, and worktree creation flow, see [tui-operations.md#add-repo](tui-operations.md#add-repo).

**Key bindings:**
- `tab`/`shift+tab` -- cycle fields (repo list, postfix input, submit button)
- `enter` -- select repo (from list) or submit (from postfix/submit)
- `q`/`escape`/`ctrl+c` -- quit
- `ctrl+d` -- quit (global)
- `q`/`escape` (progress/error state) -- quit

## Components (`internal/tui/components`)

### ListModel

```go
func NewListModel(indicatorStyle lipgloss.Style) ListModel
```

Wrapping list with variable-height items and vim navigation. Tracks cursor position and item count. The workspace list uses `ListModel` for cursor state management but renders cards directly via `renderCard()` rather than calling `ListModel.View()`.

### CheckboxListModel

```go
func NewCheckboxListModel(cursorStyle lipgloss.Style, selectedStyle lipgloss.Style) CheckboxListModel
```

Multi-select list with `[x]`/`[ ]` checkboxes, diamond cursor, and vim navigation. Items are toggled with `space`.

### TextInputModel

```go
func NewTextInputModel(focusedStyle, blurredStyle lipgloss.Style) TextInputModel
```

Single-line text input with cursor, focused/blurred styles, placeholder text, and basic editing (insert, delete, backspace, home/end, left/right).

### VimNav

```go
type VimNav struct{ gPressed bool }
func (v *VimNav) HandleKey(key string, goFirst, goLast, cursorUp, cursorDown func()) bool
```

Embeddable vim-style navigation handler. Processes `j`/`k`/`gg`/`G`/arrow keys and invokes callbacks.

### RenderFooter

```go
func RenderFooter(bindings []KeyBinding, keyStyle, descStyle, separatorStyle lipgloss.Style, width int) string
```

Pure function that renders a separator line and badge-style keybindings. Not stateful.

### KeyBinding

```go
type KeyBinding struct{ Key, Desc string }
```

Data type used by `RenderFooter` and returned by each page's `ActionBindings()` method.
