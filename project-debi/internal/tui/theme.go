package tui

import (
	"bufio"
	"image/color"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"charm.land/lipgloss/v2"
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

var defaultPalette = ThemePalette{
	Foreground: lipgloss.Color("#FAFAFA"),
	Background: lipgloss.Color("#1E1E2E"),
	Primary:    lipgloss.Color("#89B4FA"),
	Secondary:  lipgloss.Color("#CBA6F7"),
	Accent:     lipgloss.Color("#89B4FA"),
	Surface:    lipgloss.Color("#313244"),
	Panel:      lipgloss.Color("#45475A"),
	TextMuted:  lipgloss.Color("#6C7086"),
	Warning:    lipgloss.Color("#F9E2AF"),
	Error:      lipgloss.Color("#F38BA8"),
	Success:    lipgloss.Color("#A6E3A1"),
}

var colorPattern = regexp.MustCompile(`^(\S+)\s+(#[0-9A-Fa-f]{6})`)

func DefaultThemePath() string {
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

func LoadTheme(path string) ThemePalette {
	if path == "" {
		return defaultPalette
	}

	colors := parseKittyTheme(path)
	if colors == nil {
		return defaultPalette
	}

	fg, hasFg := colors["foreground"]
	bg, hasBg := colors["background"]
	if !hasFg || !hasBg {
		return defaultPalette
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
