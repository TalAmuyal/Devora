# TUI Components Package Spec

Package: `internal/tui/components`

## Purpose

Reusable UI widgets for the TUI. These are generic building blocks used by multiple page models. The package depends only on Lip Gloss for styling — it has no domain dependencies.

## Dependencies

- `charm.land/lipgloss/v2` -- styling

## File Layout

```
internal/tui/components/
  list.go           -- ListModel (variable-height wrapping list)
  checkbox_list.go  -- CheckboxListModel (multi-select checkbox list)
  textinput.go      -- TextInputModel (single-line text input)
  pathpicker.go     -- PathPickerModel (directory path picker with browser)
  vimnav.go         -- VimNav (embeddable vim key handler)
  footer.go         -- RenderFooter, KeyBinding
```

---

## KeyBinding

```go
type KeyBinding struct {
    Key  string
    Desc string
}
```

Data type for footer keybinding display. Returned by each page model's `ActionBindings()` method.

## RenderFooter

```go
func RenderFooter(bindings []KeyBinding, keyStyle, descStyle, separatorStyle lipgloss.Style, width int) string
```

Pure function that renders a separator line and badge-style keybindings. Returns an empty string if `bindings` is empty.

---

## VimNav

```go
type VimNav struct {
    gPressed bool
}
```

Embeddable vim-style navigation handler. Tracks `gg` two-key sequence state.

### HandleKey

```go
func (v *VimNav) HandleKey(key string, goFirst func(), goLast func(), cursorUp func(), cursorDown func()) bool
```

Processes navigation keys and invokes the appropriate callback:
- `j`/`down` -- `cursorDown()`
- `k`/`up` -- `cursorUp()`
- `gg` (two presses) -- `goFirst()`
- `G` -- `goLast()`

Returns `true` if the key was consumed. Resets `gPressed` on any non-`g` key.

### Reset

```go
func (v *VimNav) Reset()
```

Clears the pending `g` state. Call on focus change.

---

## ListModel

A wrapping list with variable-height items, scrolling, and a left-bar selection indicator.

### ListItem

```go
type ListItem struct {
    Content string // multi-line rendered content
    Value   any    // opaque payload
}
```

`Height()` returns the number of lines (`strings.Count(Content, "\n") + 1`).

### Type

```go
type ListModel struct {
    Items        []ListItem
    Cursor       int
    Height       int // visible area height in lines
    scrollOffset int
    vim          VimNav
    indicatorChar  string
    indicatorStyle lipgloss.Style
    itemPadding    int
}
```

### NewListModel

```go
func NewListModel(indicatorStyle lipgloss.Style) ListModel
```

Creates a list with a `▎` indicator character and 1-character left padding for non-selected items.

### Methods

- `CursorDown()` / `CursorUp()` -- Move cursor with wrapping
- `GoFirst()` / `GoLast()` -- Jump to first/last item
- `HandleKey(key string) bool` -- Delegates to embedded `VimNav`
- `SelectedItem() *ListItem` -- Returns current item or `nil` if empty
- `Reset()` -- Clears vim nav state
- `SetItems(items []ListItem)` -- Replaces items and resets cursor
- `View() string` -- Renders the visible portion with scrolling and indicator

---

## CheckboxListModel

Multi-select list with `[x]`/`[ ]` checkboxes, diamond cursor (`◆`), and vim navigation.

### Type

```go
type CheckboxListModel struct {
    Items    []string
    Selected map[int]bool
    Cursor   int
    Height   int
    vim      VimNav
    cursorStyle   lipgloss.Style
    selectedStyle lipgloss.Style
}
```

### NewCheckboxListModel

```go
func NewCheckboxListModel(cursorStyle lipgloss.Style, selectedStyle lipgloss.Style) CheckboxListModel
```

### Methods

- `CursorDown()` / `CursorUp()` / `GoFirst()` / `GoLast()` -- Navigation with wrapping
- `Toggle()` -- Toggles the selected state of the item at `Cursor`
- `SelectedItems() []string` -- Returns names of all selected items (in order)
- `HandleKey(key string) bool` -- Handles `space` (toggle) and delegates navigation to `VimNav`
- `Reset()` -- Clears vim nav state
- `View() string` -- Renders the list with cursor and checkbox indicators

---

## TextInputModel

Single-line text input with cursor, focused/blurred styles, placeholder text, and basic editing.

### Type

```go
type TextInputModel struct {
    Value       string
    Placeholder string
    Focused     bool
    cursorPos   int
    focusedStyle lipgloss.Style
    blurredStyle lipgloss.Style
    cursorStyle  lipgloss.Style
}
```

### NewTextInputModel

```go
func NewTextInputModel(focusedStyle, blurredStyle lipgloss.Style) TextInputModel
```

Creates a text input with a reverse-video cursor style.

### Methods

- `Focus()` / `Blur()` -- Set focus state
- `HandleKey(key string) bool` -- Handles editing keys (only when focused):
  - `backspace`, `delete` -- character deletion
  - `left`, `right` -- cursor movement
  - `home`/`ctrl+a`, `end`/`ctrl+e` -- jump to start/end
  - `space` -- normalised to `" "` and inserted (Bubble Tea v2 sends `"space"`, not `" "`)
  - Printable characters -- insert at cursor
- `SetValue(value string)` -- Sets value and moves cursor to end
- `View() string` -- Renders with appropriate styling based on focus and content state

---

## PathPickerModel

Directory path picker that bundles a `TextInputModel` with a directory browser. Two modes: Type Mode (text editing) and Browse Mode (directory navigation). Only shows directories (not files). Supports tilde (`~/`) expansion and preservation.

### Type

```go
type PathPickerMode int

const (
    PathPickerTypeMode   PathPickerMode = iota
    PathPickerBrowseMode
)

type PathPickerModel struct {
    textInput     TextInputModel
    mode          PathPickerMode
    Focused       bool
    entries       []string // directory names in the listed dir
    cursor        int
    browseBase    string   // resolved directory being listed
    browseErr     string   // error if listing failed
    prefix        string   // prefix filter for partial path
    tildePrefix   bool     // whether value starts with ~/
    vim           VimNav
    browserHeight int
    mutedStyle    lipgloss.Style
    cursorStyle   lipgloss.Style
}
```

### NewPathPickerModel

```go
func NewPathPickerModel(focusedStyle, blurredStyle, mutedStyle, cursorStyle lipgloss.Style, browserHeight int) PathPickerModel
```

Creates a path picker with the given styles. `browserHeight` controls how many directory entries are visible at once.

### Methods

- `Focus()` / `Blur()` -- Focus management. `Focus` always enters Type Mode
- `SetValue(value string)` -- Sets text input value and refreshes directory listing
- `Value() string` -- Returns current text input value
- `HandleKey(key string) bool` -- Mode-aware key dispatch (only when focused):
  - **Type Mode**: delegates text editing to inner `TextInputModel`. `ctrl+l` or `down` switches to Browse Mode. Typing refreshes the directory listing
  - **Browse Mode**: vim navigation (`j`/`k`/`gg`/`G`), `enter`/`l`/`right` descends into directory, `h`/`left` goes to parent, `ctrl+l` switches back to Type Mode. `up`/`k` from the top of the list switches to Type Mode. Typing a printable character switches to Type Mode and inserts it
  - **Pass-through keys** (never consumed): `tab`, `shift+tab`, `ctrl+c`, `escape`, `q`. `enter` is not consumed in Type Mode
- `Mode() PathPickerMode` -- Returns current mode
- `SetPlaceholder(placeholder string)` -- Sets text input placeholder
- `View() string` -- Renders text input. When focused, also shows a dashed separator and directory listing below. Browse Mode shows a `▸` cursor on the selected entry. When blurred, only the text input is shown

### Directory Listing

- `~/` is expanded via `os.UserHomeDir` (no domain dependencies)
- Path ending with `/` lists that directory; path without `/` lists parent directory filtered by the trailing basename as a prefix
- Only directories are shown (regular files are excluded, symlinks to directories are included)
- `..` is always the first entry (except at filesystem root `/`)
- Hidden directories (starting with `.`) are excluded
- Tilde prefix is preserved when navigating: browsing from `~/foo` into `bar` updates value to `~/foo/bar/`
