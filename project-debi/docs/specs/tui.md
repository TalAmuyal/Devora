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
func RunWorkspaceUI(themePath string, repoNames []string, showProfileRegistration bool, startupError string) (*AppResult, error)
```

Main entry point. Creates a `tea.Program` with `AppModel`, runs it, and returns the result.

- `themePath`: path to a Kitty theme file for palette extraction (empty string uses defaults)
- `repoNames`: registered repo names for the active profile
- `showProfileRegistration`: if true, starts on the profile registration page (first-run flow)
- `startupError`: if non-empty, starts on the startup error page (takes priority over `showProfileRegistration`)

Returns `*AppResult` with either `SelectedWorkspace` (user picked an existing workspace) or `NewWorkspace` (user created a new workspace). Returns `nil` if the user quit without selection.

### NewAppModel

```go
func NewAppModel(palette ThemePalette, repoNames []string, showProfileRegistration bool, startupError string) AppModel
```

Constructs the main app model. Initializes all page models, loads the active profile name, and sets the starting page. Starting page priority: `PageStartupError` if `startupError` is non-empty, then `PageProfileRegistration` if `showProfileRegistration` is true, then `PageWorkspaceList` otherwise. Called by `RunWorkspaceUI`.

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
     | PageStartupError
```

### Transitions

```
PageProfileRegistration  (first-run: entry point; normal: via settings page)
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
   +--- WorkspaceInfo (selected) ---------------> tea.Quit (with result)

PageSettings --- showProfileRegistrationMsg ---> PageProfileRegistration
             <--- showSettingsMsg -------------- (when returnToSettings is true)

PageNewTask --- NewTaskResult ---> PageCreation --- creationDoneMsg ---> tea.Quit (with result)
                                                --- creationErrorMsg --> (stays, shows error)
                                                --- back (error) -------> PageWorkspaceList

PageDeleteConfirm --- deleteCompleteMsg ---> PageWorkspaceList (+ refresh)
                  --- deleteErrorMsg ------> (stays, shows error)
```

### Navigation Keys

For the complete key binding spec covering `ctrl+c`, `esc`, `q`, and `ctrl+d` behavior across all pages and modes (insert vs navigation), see [tui-navigation.md](tui-navigation.md).

### Hub Model

`PageWorkspaceList` is the hub. All sub-pages return to it via `showWorkspaceListMsg`.

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
    startupError  StartupErrorModel

    // Shared state
    styles      Styles
    palette     ThemePalette
    repoNames   []string
    profileName string
    version     string

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
3. `openChangelogMsg` -- opens the changelog in a new Kitty tab via `openChangelogCmd`
4. Data/result messages -- handles workspace loading, creation progress, deletion, profile activation
5. `spinnerTickMsg` -- advances creation spinner frame (only while on creation page with no error)
6. `notifyMsg` / `notifyClearMsg` -- ephemeral 4-second notification system
7. `tea.KeyPressMsg` -- checks global bindings (see [tui-navigation.md](tui-navigation.md)), then delegates to the active page's `Update` method

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
|   key  Desc    key  Desc              version |  <- footer bindings + version
+----------------------------------------------+
```

## Message Types

### Page transitions

- `showWorkspaceListMsg` -- navigate to workspace list
- `refreshWorkspacesMsg` -- reload workspace data
- `showNewTaskMsg` -- navigate to new task form
- `showSettingsMsg` -- navigate to settings page
- `showRegisterRepoMsg` -- navigate to register repo page
- `showProfileRegistrationMsg{fromSettings}` -- navigate to profile registration page. `fromSettings` controls whether back/cancel returns to the settings page instead of the workspace list
- `openChangelogMsg` -- open the changelog in a new Kitty tab

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

Displays workspaces as styled cards. Each card shows task title (or workspace name), a category badge (ACTIVE/IDLE/INVALID), and a per-repo table with branch name and clean/dirty status. Selection is indicated by the `ListModel` bar indicator and a distinct card border style. Cards are pre-rendered into `ListItem.Content` and the `View()` method delegates to `ListModel.View()` for scrollable, viewport-constrained rendering. On terminal resize, card content is refreshed at the new width without resetting the cursor.

**Key bindings:**
- `1`/`2`/`3` -- switch filter (Active, Inactive, All)
- `j`/`k`/`gg`/`G`/arrows -- vim navigation
- `enter` -- select workspace (exits TUI with result)
- `n` -- new task, `d` -- deactivate (active workspaces only), `D` -- delete (inactive/invalid workspaces only), `r` -- refresh
- `R` -- register repo, `p` -- cycle profile
- `s` -- settings
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` (delegates navigation to embedded `ListModel`).

**Transitions:** Emits `showNewTaskMsg`, `showSettingsMsg`, `showRegisterRepoMsg`, `refreshWorkspacesMsg`, `requestDeleteMsg`, `deactivateCompleteMsg`/`deactivateErrorMsg`, `WorkspaceInfo` (selection), or `tea.Quit`.

### NewTaskModel

```go
func NewNewTaskModel(repoNames []string, styles *Styles) NewTaskModel
```

Multi-field form with three tab stops: repo selection (checkbox list), task name (text input), and submit button. Validates that at least one repo is selected and the task name is non-empty.

**Key bindings:**
- `tab`/`shift+tab` -- cycle fields
- `space` -- toggle repo checkbox (when repos focused)
- `enter` -- submit (when on task name or submit field)
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `NewTaskResult` on submit, `showWorkspaceListMsg` on cancel.

### CreationModel

```go
func NewCreationModel(styles *Styles) CreationModel
```

Read-only progress display. Shows status text and per-repo progress (preparing/ready) with an animated Braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`, 80ms interval) on repos still preparing. On error, displays error message with option to go back.

The spinner is driven by `spinnerTickMsg`, handled by `AppModel.Update`: it advances the `spinnerFrame` index and schedules the next tick. The tick stops when creation completes or errors.

**Key bindings (error state only):**
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` only. Creation progress messages (`creationStatusMsg`, `repoAddedMsg`, `repoReadyMsg`, `creationDoneMsg`, `creationErrorMsg`) and `spinnerTickMsg` are handled by `AppModel.Update`, which updates `CreationModel` fields directly.

**Transitions:** Emits `showWorkspaceListMsg` on error dismissal.

### DeleteConfirmModel

```go
func NewDeleteConfirmModel(styles *Styles) DeleteConfirmModel
```

Centered dialog showing workspace name, path, and a warning about the destructive operation.

**Key bindings:**
- `y` -- confirm deletion
- `n` -- cancel, back to workspace list
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` only. `deleteErrorMsg` is handled by `AppModel.Update`, which updates `DeleteConfirmModel.errMsg` directly.

**Transitions:** Emits `deleteCompleteMsg` or `deleteErrorMsg` after deletion attempt. Emits `showWorkspaceListMsg` on cancel. For delete blocking rules (active session, non-HEAD branch, dirty repo), see [tui-operations.md#delete-blocking-rules](tui-operations.md#delete-blocking-rules).

### RegisterRepoModel

```go
func NewRegisterRepoModel(styles *Styles) RegisterRepoModel
```

Path picker (PathPickerModel with directory browser) for entering a git repo path. The path is tilde-expanded (via `config.ExpandTilde`) before validation. Validates that the path exists, is a directory, and is a git repository before registering.

**Key bindings:**
- `ctrl+l` -- toggle between typing and browsing directories
- `enter` -- register the repo (type mode); descend into directory (browse mode)
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `notifyMsg` (success) + `showWorkspaceListMsg` on success, stays on page with error on failure.

### SettingsModel

```go
func NewSettingsModel(styles *Styles) SettingsModel
```

Multi-item settings page with focusable fields navigated via `j`/`k` in navigation mode. Fields are organized into sections:

**Configuration section:**
1. **Default App (Global)** (`fieldDefaultAppGlobal`): Inline-editable text input for `"terminal.default-app"` at the global scope. Press `enter` to start editing; `enter` saves, `esc` cancels and restores the previous value. An empty input on save clears the key (stores `nil`); a non-empty input stores the value. The reserved value `"shell"` means "bare login/interactive shell, no wrapped command".
2. **Default App (Profile)** (`fieldDefaultAppProfile`): Same behavior as the global field, but writes to the active profile's config. An empty input on save clears the key so the global value takes effect. The reserved value `"shell"` means "bare login/interactive shell, no wrapped command".
3. **Prepare Command** (`fieldPrepareCmd`): Inline-editable text input for the shell command run after worktree creation. Press `enter` to start editing (enters editing mode); `enter` saves, `esc` cancels and restores the previous value.

**Repos section:**
4. **Add Repo** (`fieldAddRepo`): Press `enter` to navigate to the register repo page.
5. **Remove Repo** (dynamic, one per explicit repo): Press `enter` to show an inline y/n confirmation prompt.

**Info section:**
6. **View Changelog** (`viewChangelogField()`): Press `enter` to open the changelog in a new Kitty tab (emits `openChangelogMsg`).

**Profiles section:**
7. **Add Profile** (`addProfileField()`): Press `enter` to navigate to the profile registration page (emits `showProfileRegistrationMsg{fromSettings: true}`).
8. **Delete Profile** (`deleteProfileField()`): Press `enter` to show an inline y/n confirmation prompt. `y` unregisters the active profile; if no profiles remain, quits the app. If other profiles remain, activates the first one and emits `profileActivatedMsg`. `n` or `esc` cancels.

Loads the current values for the three editable fields on activation: `GetDefaultTerminalAppGlobalRaw()`, `GetDefaultTerminalAppProfileRaw()`, and `GetPrepareCommand()`. Title includes profile name when available.

The page has three modes:
- **Navigation mode**: `j`/`k` moves focus between fields, `enter` activates the focused field, `esc`/`q` goes back.
- **Editing mode** (default-app global, default-app profile, or prepare command): Text input is focused. `enter` saves, `esc` cancels and reverts.
- **Confirm-delete mode**: Shows y/n prompt. `y` confirms, `n`/`esc` cancels.

**Key bindings (navigation mode):**
- `j`/`k`/arrows -- move focus between fields
- `enter` -- activate focused field
- `esc`/`q` -- back to workspace list

**Key bindings (editing mode):**
- `enter` -- save settings
- `esc` -- cancel editing, revert value

**Key bindings (confirm-delete mode):**
- `y` -- confirm deletion
- `n`/`esc` -- cancel

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** On save success, emits `notifyMsg` (success) and stays on page. On save error, emits `notifyMsg` (error) and stays on page. On delete, emits `profileActivatedMsg` (if profiles remain) or `tea.Quit` (if last profile). Emits `openChangelogMsg` for View Changelog. Emits `showProfileRegistrationMsg{fromSettings: true}` for Add Profile. Emits `showWorkspaceListMsg` on back.

### ProfileRegModel

```go
func NewProfileRegModel(styles *Styles, hasBack bool) ProfileRegModel
```

Path picker (PathPickerModel with directory browser) + name text input + submit button, with three tab stops. An explanatory label at the top describes what will be created in the chosen directory (`config.json`, `repos/`, `workspaces/`). Has a `returnToSettings` field set by `AppModel` when navigating from the settings page. Behavior varies based on `hasBack` and `returnToSettings`:
- First-run (`hasBack=false`): cancel quits the app entirely
- Normal (`hasBack=true`, `returnToSettings=false`): cancel returns to workspace list
- From settings (`hasBack=true`, `returnToSettings=true`): cancel returns to settings page

Validates that path is provided, and name is required for uninitialized profiles. On success, activates the profile (emits `profileActivatedMsg`, which always navigates to workspace list regardless of origin).

**Key bindings:**
- `tab`/`shift+tab` -- cycle fields
- `ctrl+l` -- toggle between typing and browsing directories (path field)
- `enter` -- register (when on name or submit field); descend into directory (when on path field in browse mode)
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `profileActivatedMsg` on success. On cancel: emits `showSettingsMsg` (when `returnToSettings` is true), `showWorkspaceListMsg` (when `hasBack` is true), or `tea.Quit` (first-run).

### StartupErrorModel

```go
func NewStartupErrorModel(styles *Styles, message string) StartupErrorModel
```

Full-screen error page shown when PATH is misconfigured (bundled tools directory missing from PATH). Displays the provided error message. Only handles quit keys (`ctrl+c`, `q`).

**Key bindings:**
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

**Messages handled:** `tea.KeyPressMsg` only.

**Transitions:** Emits `tea.Quit` on quit keys.

## Standalone Models

### AddRepoModel

`AddRepoModel` is a standalone `tea.Model` (implements `Init`, `Update`, `View`) run via `RunAddRepo` in its own `tea.Program`. It is NOT embedded in `AppModel`. For its form fields, collision checking, and worktree creation flow, see [tui-operations.md#add-repo](tui-operations.md#add-repo).

**Key bindings:**
- `tab`/`shift+tab` -- cycle fields (repo list, postfix input, submit button)
- `enter` -- select repo (from list) or submit (from postfix/submit)
- Navigation keys: see [tui-navigation.md](tui-navigation.md)

## Components (`internal/tui/components`)

### ListModel

```go
func NewListModel(indicatorStyle lipgloss.Style) ListModel
```

Wrapping list with variable-height items and vim navigation. Tracks cursor position and item count. The workspace list populates `ListItem.Content` with rendered cards and delegates to `ListModel.View()` for viewport-constrained scrolling. `View()` updates `Content` with selection-aware styling on each render, while `rebuildListItems()` populates initial `Content` for correct `Height()` calculations.

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

### PathPickerModel

```go
func NewPathPickerModel(focusedStyle, blurredStyle, mutedStyle, cursorStyle lipgloss.Style, browserHeight int) PathPickerModel
```

Directory path picker that bundles a `TextInputModel` with a directory browser. Two modes: Type Mode (text editing) and Browse Mode (directory navigation with vim keys). Shows only directories, supports tilde preservation, and passes through `tab`/`shift+tab`/`ctrl+c` to the parent form.

### VimNav

```go
type VimNav struct{ gPressed bool }
func (v *VimNav) HandleKey(key string, goFirst, goLast, cursorUp, cursorDown func()) bool
```

Embeddable vim-style navigation handler. Processes `j`/`k`/`gg`/`G`/arrow keys and invokes callbacks.

### RenderFooter

```go
func RenderFooter(bindings []KeyBinding, keyStyle, descStyle, separatorStyle lipgloss.Style, width int, versionText string) string
```

Pure function that renders a separator line and badge-style keybindings. If `versionText` is non-empty, it is rendered right-aligned on the keybinding row. Not stateful.

### KeyBinding

```go
type KeyBinding struct{ Key, Desc string }
```

Data type used by `RenderFooter` and returned by each page's `ActionBindings()` method.
