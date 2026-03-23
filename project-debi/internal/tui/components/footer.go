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
func RenderFooter(bindings []KeyBinding, keyStyle, descStyle, separatorStyle lipgloss.Style, width int) string {
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

	return sep + "\n" + row
}
