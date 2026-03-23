# Theme

Package: `internal/tui` (files: `theme.go`, `styles.go`)

## Purpose

The theme system extracts a color palette from a Kitty terminal theme file and builds all Lip Gloss styles from it. This lets Devora's TUI inherit the user's terminal color scheme automatically, while providing sensible hardcoded defaults when no theme file is available.

For how the styles are consumed by TUI components, see [specs/tui.md](tui.md).

## ThemePalette

```go
type ThemePalette struct {
    Foreground color.Color
    Background color.Color
    Primary    color.Color
    Secondary  color.Color
    Accent     color.Color
    Surface    color.Color
    Panel      color.Color
    TextMuted  color.Color
    Warning    color.Color
    Error      color.Color
    Success    color.Color
}
```

All fields are `color.Color` (from `image/color`).

### Kitty Color Source Mapping

When loading from a Kitty theme file, each palette field is sourced from a specific Kitty color key. If the key is missing, the fallback column shows what is used instead.

| Palette Field | Kitty Theme Key | Fallback |
|---|---|---|
| `Foreground` | `foreground` | (required) |
| `Background` | `background` | (required) |
| `Primary` | `active_tab_background` | foreground value |
| `Secondary` | `color5` (magenta) | foreground value |
| `Accent` | `color4` (blue) | foreground value |
| `Surface` | `inactive_tab_background` | background value |
| `Panel` | `color0` (black) | background value |
| `TextMuted` | `color8` (bright black) | foreground value |
| `Warning` | `color3` (yellow) | foreground value |
| `Error` | `color1` (red) | foreground value |
| `Success` | `color2` (green) | foreground value |

## Default Palette

When no theme file is found (or the file is missing required keys), these hardcoded values are used:

| Field | Hex |
|---|---|
| `Foreground` | `#FAFAFA` |
| `Background` | `#1E1E2E` |
| `Primary` | `#89B4FA` |
| `Secondary` | `#CBA6F7` |
| `Accent` | `#89B4FA` |
| `Surface` | `#313244` |
| `Panel` | `#45475A` |
| `TextMuted` | `#6C7086` |
| `Warning` | `#F9E2AF` |
| `Error` | `#F38BA8` |
| `Success` | `#A6E3A1` |

## LoadTheme

```go
func LoadTheme(path string) ThemePalette
```

Parses a Kitty theme file and returns a resolved palette.

Behavior:
1. If `path` is empty, return the default palette.
2. Parse the file line by line. Skip blank lines and lines starting with `#` (comments). Extract key-value pairs matching the regex `^(\S+)\s+(#[0-9A-Fa-f]{6})`. Hex values are uppercased.
3. If the file cannot be opened, return the default palette.
4. If both `foreground` and `background` are present, build a `ThemePalette` using the mapping table above (with per-field fallbacks for missing keys).
5. If either `foreground` or `background` is missing, return the default palette entirely.

## DefaultThemePath

```go
func DefaultThemePath() string
```

Returns `~/.config/kitty/current-theme.conf` (using `os.UserHomeDir()` for the home directory). Returns an empty string if the home directory cannot be determined.

## Styles

```go
type Styles struct {
    // Header
    Brand           lipgloss.Style
    HeaderDot       lipgloss.Style
    HeaderPageTitle lipgloss.Style
    HeaderProfile   lipgloss.Style
    Separator       lipgloss.Style

    // Tab bar
    TabActive   lipgloss.Style
    TabInactive lipgloss.Style

    // Workspace cards
    CardSelected   lipgloss.Style
    CardUnselected lipgloss.Style
    CardTitle      lipgloss.Style
    Diamond        lipgloss.Style
    BadgeActive    lipgloss.Style
    BadgeIdle      lipgloss.Style
    BadgeInvalid   lipgloss.Style
    RepoName       lipgloss.Style
    RepoBranch     lipgloss.Style
    RepoClean      lipgloss.Style
    RepoDirty      lipgloss.Style

    // Footer
    FooterKey  lipgloss.Style
    FooterDesc lipgloss.Style

    // Forms
    FieldLabelFocused lipgloss.Style
    FieldLabelBlurred lipgloss.Style
    SubmitFocused     lipgloss.Style
    SubmitBlurred     lipgloss.Style

    // Checkbox
    CheckboxCursor      lipgloss.Style
    CheckboxCheckedMark lipgloss.Style

    // Notifications
    NotifyError   lipgloss.Style
    NotifySuccess lipgloss.Style

    // Dialog (delete confirmation)
    DialogBorder lipgloss.Style

    // General text
    Title lipgloss.Style
    Muted lipgloss.Style

    // Colors for direct use
    AccentColor  color.Color
    SuccessColor color.Color
    ErrorColor   color.Color
    WarningColor color.Color
    PanelColor   color.Color
}
```

### Style Details by Zone

**Header**

| Field | Palette Color | Properties |
|---|---|---|
| `Brand` | `Primary` | Bold |
| `HeaderDot` | `TextMuted` | |
| `HeaderPageTitle` | `Secondary` | Bold |
| `HeaderProfile` | `Secondary` | |
| `Separator` | `TextMuted` | |

**Tab bar**

| Field | Palette Color | Properties |
|---|---|---|
| `TabActive` | `Accent` | Bold |
| `TabInactive` | `TextMuted` | |

**Workspace cards**

| Field | Palette Color | Properties |
|---|---|---|
| `CardSelected` | `Accent` (border) | Rounded border, padding 0/2, margin-left 2 |
| `CardUnselected` | `TextMuted` (border) | Normal border, padding 0/2, margin-left 2 |
| `CardTitle` | `Foreground` | Bold |
| `Diamond` | `Accent` | Bold |
| `BadgeActive` | `Success` | Bold |
| `BadgeIdle` | `TextMuted` | |
| `BadgeInvalid` | `Error` | Bold |
| `RepoName` | `Foreground` | |
| `RepoBranch` | `TextMuted` | |
| `RepoClean` | `Success` | |
| `RepoDirty` | `Warning` | |

**Footer**

| Field | Palette Color | Properties |
|---|---|---|
| `FooterKey` | `Accent` | Bold |
| `FooterDesc` | `Foreground` | |

**Forms**

| Field | Palette Color | Properties |
|---|---|---|
| `FieldLabelFocused` | `Secondary` | Bold |
| `FieldLabelBlurred` | `TextMuted` | |
| `SubmitFocused` | `Accent` | Bold |
| `SubmitBlurred` | `TextMuted` | |

**Checkbox**

| Field | Palette Color | Properties |
|---|---|---|
| `CheckboxCursor` | `Accent` | Bold |
| `CheckboxCheckedMark` | `Accent` | |

**Notifications**

| Field | Palette Color | Properties |
|---|---|---|
| `NotifyError` | `Error` | Padding-left 2 |
| `NotifySuccess` | `Success` | Padding-left 2 |

**Dialog**

| Field | Palette Color | Properties |
|---|---|---|
| `DialogBorder` | `Warning` (border) | Double border, padding 1/2 |

**General text**

| Field | Palette Color | Properties |
|---|---|---|
| `Title` | `Foreground` | Bold |
| `Muted` | `TextMuted` | |

### Direct Color References

Five palette colors are exposed directly on `Styles` for use cases where a `lipgloss.Style` is not needed (e.g., conditional coloring logic):

| Field | Source |
|---|---|
| `AccentColor` | `palette.Accent` |
| `SuccessColor` | `palette.Success` |
| `ErrorColor` | `palette.Error` |
| `WarningColor` | `palette.Warning` |
| `PanelColor` | `palette.Panel` |

## NewStyles

```go
func NewStyles(palette ThemePalette) Styles
```

Constructs a `Styles` value from a `ThemePalette`. Every style field is initialized from the palette — there are no hardcoded colors in `NewStyles`. Called once during app initialization.
