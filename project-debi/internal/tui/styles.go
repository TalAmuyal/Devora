package tui

import (
	"image/color"

	"charm.land/lipgloss/v2"
)

// Styles holds all Lip Gloss styles for the panel-based dashboard design.
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

func NewStyles(palette ThemePalette) Styles {
	return Styles{
		// Header
		Brand: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Primary),

		HeaderDot: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		HeaderPageTitle: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Secondary),

		HeaderProfile: lipgloss.NewStyle().
			Foreground(palette.Secondary),

		Separator: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		// Tab bar
		TabActive: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Accent),

		TabInactive: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		// Workspace cards
		CardSelected: lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(palette.Accent).
			Padding(0, 2).
			MarginLeft(2),

		CardUnselected: lipgloss.NewStyle().
			Border(lipgloss.NormalBorder()).
			BorderForeground(palette.TextMuted).
			Padding(0, 2).
			MarginLeft(2),

		CardTitle: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Foreground),

		Diamond: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Accent),

		BadgeActive: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Success),

		BadgeIdle: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		BadgeInvalid: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Error),

		RepoName: lipgloss.NewStyle().
			Foreground(palette.Foreground),

		RepoBranch: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		RepoClean: lipgloss.NewStyle().
			Foreground(palette.Success),

		RepoDirty: lipgloss.NewStyle().
			Foreground(palette.Warning),

		// Footer
		FooterKey: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Accent),

		FooterDesc: lipgloss.NewStyle().
			Foreground(palette.Foreground),

		// Forms
		FieldLabelFocused: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Secondary),

		FieldLabelBlurred: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		SubmitFocused: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Accent),

		SubmitBlurred: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		// Checkbox
		CheckboxCursor: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Accent),

		CheckboxCheckedMark: lipgloss.NewStyle().
			Foreground(palette.Accent),

		// Notifications
		NotifyError: lipgloss.NewStyle().
			Foreground(palette.Error).
			PaddingLeft(2),

		NotifySuccess: lipgloss.NewStyle().
			Foreground(palette.Success).
			PaddingLeft(2),

		// Dialog
		DialogBorder: lipgloss.NewStyle().
			Border(lipgloss.DoubleBorder()).
			BorderForeground(palette.Warning).
			Padding(1, 2),

		// General text
		Title: lipgloss.NewStyle().
			Bold(true).
			Foreground(palette.Foreground),

		Muted: lipgloss.NewStyle().
			Foreground(palette.TextMuted),

		// Colors
		AccentColor:  palette.Accent,
		SuccessColor: palette.Success,
		ErrorColor:   palette.Error,
		WarningColor: palette.Warning,
		PanelColor:   palette.Panel,
	}
}
