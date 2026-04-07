package tui

import (
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/tui/components"
)

// StartupErrorModel is a full-screen error page shown when the app detects
// a critical startup problem (e.g. bundled-apps directory missing from PATH).
// It blocks normal usage until the user fixes their shell config.
type StartupErrorModel struct {
	message string
	styles  *Styles
	width   int
	height  int
}

func NewStartupErrorModel(styles *Styles, message string) StartupErrorModel {
	return StartupErrorModel{
		message: message,
		styles:  styles,
	}
}

// Update handles key events. Only quit keys (q, esc, ctrl+d) are accepted.
func (m *StartupErrorModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		switch key {
		case "q", "esc", "ctrl+d":
			return tea.Quit
		}
	}
	return nil
}

// View renders the startup error page with the error message and instructions.
func (m *StartupErrorModel) View() string {
	var b strings.Builder

	title := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.styles.ErrorColor).
		Render("\u26a0 PATH Configuration Error")

	b.WriteString("\n")
	b.WriteString("  " + title)
	b.WriteString("\n\n")
	b.WriteString("  Devora's bundled tools directory is not in your shell's PATH.\n")
	b.WriteString("  This usually happens when shell startup files reset the PATH variable.\n")
	b.WriteString("\n")

	for _, line := range strings.Split(m.message, "\n") {
		b.WriteString("  " + line + "\n")
	}

	instructions := []string{
		"To fix this, check your ~/.zprofile and ~/.zshrc files and ensure",
		"they do not unconditionally overwrite the PATH variable.",
		"",
		"After fixing the configuration, restart Devora.",
	}
	for _, line := range instructions {
		b.WriteString("  " + m.styles.Muted.Render(line) + "\n")
	}

	return b.String()
}

// SetSize updates the available width and height.
func (m *StartupErrorModel) SetSize(w, h int) {
	m.width = w
	m.height = h
}

// ActionBindings returns the keybindings shown in the footer.
func (m StartupErrorModel) ActionBindings() []components.KeyBinding {
	return []components.KeyBinding{
		{Key: "q", Desc: "Quit"},
		{Key: "esc", Desc: "Quit"},
	}
}

// borderTitle returns the title displayed in the header.
func (m StartupErrorModel) borderTitle() string {
	return "Startup Error"
}
