package style

import "charm.land/lipgloss/v2"

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
// as its hue counterpart.
var (
	Success = Green
	Error   = Red
	Warning = Yellow
	Info    = Cyan
)
