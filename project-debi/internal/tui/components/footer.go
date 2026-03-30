package components

import (
	"strings"

	"charm.land/lipgloss/v2"
)

// KeyBinding represents a keybinding displayed in the footer.
type KeyBinding struct {
	Key  string
	Desc string
}

// RenderFooter renders a footer with a separator line and badge-style keybindings.
// If versionText is non-empty, it is rendered right-aligned on the keybinding row.
func RenderFooter(bindings []KeyBinding, keyStyle, descStyle, separatorStyle lipgloss.Style, width int, versionText string) string {
	if len(bindings) == 0 {
		return ""
	}

	// Separator line
	sepWidth := width - 4
	if sepWidth < 0 {
		sepWidth = 0
	}
	sep := "  " + separatorStyle.Render(strings.Repeat("\u2500", sepWidth))

	// Keybindings: "  key  Desc    key  Desc    ..."
	var parts []string
	for _, b := range bindings {
		parts = append(parts, keyStyle.Render(b.Key)+"  "+descStyle.Render(b.Desc))
	}
	row := "   " + strings.Join(parts, "    ")

	if versionText != "" {
		versionStr := separatorStyle.Render(versionText)
		rowWidth := lipgloss.Width(row)
		versionWidth := lipgloss.Width(versionStr)
		gap := width - rowWidth - versionWidth
		if gap < 2 {
			gap = 2
		}
		row = row + strings.Repeat(" ", gap) + versionStr
	}

	return sep + "\n" + row
}
