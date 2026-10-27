// Package style holds the Catppuccin Mocha lipgloss styles used for colored CLI output.
package style

import "charm.land/lipgloss/v2"

// Catppuccin Mocha hex constants. Private — call sites should consume the
// pre-built styles below rather than reaching for raw hex.
const (
	mochaOverlay0 = "#6C7086"
	mochaGreen    = "#A6E3A1"
	mochaRed      = "#F38BA8"
	mochaYellow   = "#F9E2AF"
	mochaSky      = "#89DCEB"
)

// Lower layer: hue-named pre-built styles. Use when the call site is rendering
// a state or decoration whose color matters more than its semantic role.
var (
	Green  = lipgloss.NewStyle().Foreground(lipgloss.Color(mochaGreen))
	Red    = lipgloss.NewStyle().Foreground(lipgloss.Color(mochaRed))
	Yellow = lipgloss.NewStyle().Foreground(lipgloss.Color(mochaYellow))
	Cyan   = lipgloss.NewStyle().Foreground(lipgloss.Color(mochaSky))
	Muted  = lipgloss.NewStyle().Foreground(lipgloss.Color(mochaOverlay0))
)

// Higher layer: semantic aliases. Use when the call site is asserting an
// outcome (success/failure/etc.). Each alias is the same lipgloss.Style value
// as its hue counterpart. Muted has no alias: de-emphasized output always uses
// the hue name.
var (
	Success = Green
	Error   = Red
	Warning = Yellow
	Info    = Cyan
)
