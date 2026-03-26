package tui

import (
	"fmt"
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/config"
	"devora/internal/tui/components"
)

// SettingsModel is the settings form page with a single field: prepare command.
type SettingsModel struct {
	prepareCmd  components.TextInputModel
	navMode     bool
	styles      *Styles
	profileName string
	width       int
}

func NewSettingsModel(styles *Styles) SettingsModel {
	prepareCmd := components.NewTextInputModel(
		lipgloss.NewStyle().Foreground(styles.AccentColor),
		styles.Muted,
	)
	prepareCmd.Placeholder = "Shell command to run after worktree creation..."
	prepareCmd.Focus()

	return SettingsModel{
		prepareCmd: prepareCmd,
		styles:     styles,
	}
}

// Activate loads the current prepare-command value and sets the profile name for the title.
func (m *SettingsModel) Activate(profileName string) {
	m.profileName = profileName
	current := config.GetPrepareCommand()
	if current != nil {
		m.prepareCmd.SetValue(*current)
	} else {
		m.prepareCmd.SetValue("")
	}
	m.prepareCmd.Focus()
	m.navMode = false
}

// Update handles key events for the settings page.
func (m *SettingsModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		// Two-stage esc/q navigation
		if key == "esc" || key == "q" {
			return m.handleEscOrQ(key)
		}

		switch key {
		case "enter":
			return m.save()
		}

		// Any other key in nav mode: re-focus and exit nav mode
		if m.navMode {
			m.navMode = false
			m.prepareCmd.Focus()
		}

		m.prepareCmd.HandleKey(key)
	}
	return nil
}

// handleEscOrQ implements two-stage esc navigation.
func (m *SettingsModel) handleEscOrQ(key string) tea.Cmd {
	if m.navMode {
		return func() tea.Msg { return showWorkspaceListMsg{} }
	}

	// Insert mode
	if key == "esc" {
		m.navMode = true
		m.prepareCmd.Blur()
		return nil
	}
	// q types the letter
	m.prepareCmd.HandleKey(key)
	return nil
}

func (m *SettingsModel) save() tea.Cmd {
	value := strings.TrimSpace(m.prepareCmd.Value)
	var valuePtr *string
	if value != "" {
		valuePtr = &value
	}
	err := config.SetPrepareCommand(valuePtr)
	if err != nil {
		return func() tea.Msg {
			return notifyMsg{text: fmt.Sprintf("Error saving settings: %v", err), isError: true}
		}
	}
	return tea.Batch(
		func() tea.Msg { return notifyMsg{text: "Settings saved", isError: false} },
		func() tea.Msg { return showWorkspaceListMsg{} },
	)
}

// View renders the settings form.
func (m *SettingsModel) View() string {
	var b strings.Builder

	bar := lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
	label := m.styles.FieldLabelFocused.Render("Prepare Command")

	b.WriteString("\n")
	b.WriteString("  " + bar + label + "\n")
	b.WriteString("  " + bar + m.prepareCmd.View() + "\n")

	return b.String()
}

// SetSize updates the width and height available.
func (m *SettingsModel) SetSize(w, h int) {
	m.width = w
}

// ActionBindings returns the keybindings for the footer.
func (m SettingsModel) ActionBindings() []components.KeyBinding {
	bindings := []components.KeyBinding{
		{Key: "enter", Desc: "Save"},
	}

	if m.navMode {
		bindings = append(bindings, components.KeyBinding{Key: "esc/q", Desc: "Back"})
	} else {
		bindings = append(bindings, components.KeyBinding{Key: "esc", Desc: "Unfocus"})
	}

	return bindings
}

// borderTitle returns the title displayed in the border.
func (m SettingsModel) borderTitle() string {
	if m.profileName != "" {
		return fmt.Sprintf("Settings . %s", m.profileName)
	}
	return "Settings"
}
