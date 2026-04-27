package style

import (
	"bufio"
	"image/color"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"charm.land/lipgloss/v2"
)

// Catppuccin Mocha hex constants. Private — call sites should consume the
// pre-built styles in styles.go (or, for the TUI palette, DefaultPalette /
// LoadKittyTheme below) rather than reaching for raw hex.
const (
	mochaText     = "#CDD6F4"
	mochaBase     = "#1E1E2E"
	mochaSurface0 = "#313244"
	mochaSurface1 = "#45475A"
	mochaOverlay0 = "#6C7086"
	mochaBlue     = "#89B4FA"
	mochaMauve    = "#CBA6F7"
	mochaGreen    = "#A6E3A1"
	mochaRed      = "#F38BA8"
	mochaYellow   = "#F9E2AF"
	mochaSky      = "#89DCEB"
)

// ThemePalette holds resolved colors for the TUI theme.
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

// DefaultPalette is the fallback used when no Kitty theme file is available
// (or when a file lacks the required foreground/background keys). The
// Foreground value is intentionally #FAFAFA — an outlier from canonical
// Mocha Text — preserved as-is pending a separate review.
var DefaultPalette = ThemePalette{
	Foreground: lipgloss.Color("#FAFAFA"),
	Background: lipgloss.Color(mochaBase),
	Primary:    lipgloss.Color(mochaBlue),
	Secondary:  lipgloss.Color(mochaMauve),
	Accent:     lipgloss.Color(mochaBlue),
	Surface:    lipgloss.Color(mochaSurface0),
	Panel:      lipgloss.Color(mochaSurface1),
	TextMuted:  lipgloss.Color(mochaOverlay0),
	Warning:    lipgloss.Color(mochaYellow),
	Error:      lipgloss.Color(mochaRed),
	Success:    lipgloss.Color(mochaGreen),
}

var colorPattern = regexp.MustCompile(`^(\S+)\s+(#[0-9A-Fa-f]{6})`)

func DefaultKittyThemePath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".config", "kitty", "current-theme.conf")
}

func parseKittyTheme(path string) map[string]string {
	f, err := os.Open(path)
	if err != nil {
		return nil
	}
	defer f.Close()

	colors := make(map[string]string)
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		matches := colorPattern.FindStringSubmatch(line)
		if matches != nil {
			colors[matches[1]] = strings.ToUpper(matches[2])
		}
	}
	return colors
}

func LoadKittyTheme(path string) ThemePalette {
	if path == "" {
		return DefaultPalette
	}

	colors := parseKittyTheme(path)
	if colors == nil {
		return DefaultPalette
	}

	fg, hasFg := colors["foreground"]
	bg, hasBg := colors["background"]
	if !hasFg || !hasBg {
		return DefaultPalette
	}

	colorOrFallback := func(key string, fallback string) color.Color {
		if c, ok := colors[key]; ok {
			return lipgloss.Color(c)
		}
		return lipgloss.Color(fallback)
	}

	return ThemePalette{
		Foreground: lipgloss.Color(fg),
		Background: lipgloss.Color(bg),
		Primary:    colorOrFallback("active_tab_background", fg),
		Secondary:  colorOrFallback("color5", fg),
		Accent:     colorOrFallback("color4", fg),
		Surface:    colorOrFallback("inactive_tab_background", bg),
		Panel:      colorOrFallback("color0", bg),
		TextMuted:  colorOrFallback("color8", fg),
		Warning:    colorOrFallback("color3", fg),
		Error:      colorOrFallback("color1", fg),
		Success:    colorOrFallback("color2", fg),
	}
}
