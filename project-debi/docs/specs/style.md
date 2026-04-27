# Style

Package: `internal/style` (files: `palette.go`, `styles.go`)

## Purpose

The style package owns Devora's Catppuccin Mocha color palette and exposes it
via two complementary surfaces:

1. A `ThemePalette` value loaded from a Kitty theme file (with a hardcoded
   fallback). Consumed by the TUI to derive every Lip Gloss style in
   `internal/tui` (see [tui.md](tui.md) for how `internal/tui.NewStyles`
   composes the palette into a `Styles` struct).
2. A set of pre-built `lipgloss.Style` values intended for CLI commands that
   render colored progress / status output (`internal/prstatus`,
   `internal/submit`, `internal/close`, `internal/health`,
   `internal/workspace/wsgit`).

The two surfaces share the same Mocha hex constants but are otherwise
independent. CLI consumers do not need a `ThemePalette`; they pull individual
`lipgloss.Style` values from `internal/style` directly.

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

## DefaultPalette

When no theme file is found (or the file is missing required keys), this hardcoded palette is used:

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

`Foreground` is `#FAFAFA`, an outlier from canonical Mocha Text (`#CDD6F4`).
This value is preserved as-is and is the subject of a separate follow-up
question; do not "fix" it without that decision.

## LoadKittyTheme

```go
func LoadKittyTheme(path string) ThemePalette
```

Parses a Kitty theme file and returns a resolved palette.

Behavior:
1. If `path` is empty, return `DefaultPalette`.
2. Parse the file line by line. Skip blank lines and lines starting with `#` (comments). Extract key-value pairs matching the regex `^(\S+)\s+(#[0-9A-Fa-f]{6})`. Hex values are uppercased.
3. If the file cannot be opened, return `DefaultPalette`.
4. If both `foreground` and `background` are present, build a `ThemePalette` using the mapping table above (with per-field fallbacks for missing keys).
5. If either `foreground` or `background` is missing, return `DefaultPalette` entirely.

## DefaultKittyThemePath

```go
func DefaultKittyThemePath() string
```

Returns `~/.config/kitty/current-theme.conf` (using `os.UserHomeDir()` for the home directory). Returns an empty string if the home directory cannot be determined.

## CLI styles

For commands that render colored CLI output, `internal/style` exposes
pre-built `lipgloss.Style` values. The package presents them in two layers:

### Hue layer

Use when the call site is rendering a state or decoration whose color matters
more than its semantic role (e.g., a PR state token, a count cell).

| Variable | Mocha hex |
|---|---|
| `Green` | `#A6E3A1` |
| `Red` | `#F38BA8` |
| `Yellow` | `#F9E2AF` |
| `Cyan` | `#89DCEB` |
| `Muted` | `#6C7086` |

### Semantic layer

Use when the call site is asserting an outcome (success/failure/etc.). Each
alias is the same `lipgloss.Style` value as its hue counterpart, copied at
package init.

| Variable | Aliases |
|---|---|
| `Success` | `Green` |
| `Error` | `Red` |
| `Warning` | `Yellow` |
| `Info` | `Cyan` |

There is no semantic alias for `Muted` — informational/de-emphasized output
is always referenced by the hue name.
