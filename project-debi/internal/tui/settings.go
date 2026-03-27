package tui

import (
	"fmt"
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/config"
	"devora/internal/tui/components"
)

type settingsField int

const (
	fieldPrepareCmd    settingsField = iota
	fieldAddProfile
	fieldDeleteProfile
)

const settingsFieldCount = 3

type SettingsModel struct {
	prepareCmd    components.TextInputModel
	focused       settingsField
	editing       bool
	confirmDelete bool
	styles        *Styles
	profileName   string
	width         int
}

func NewSettingsModel(styles *Styles) SettingsModel {
	prepareCmd := components.NewTextInputModel(
		lipgloss.NewStyle().Foreground(styles.AccentColor),
		styles.Muted,
	)
	prepareCmd.Placeholder = "Shell command to run after worktree creation..."

	return SettingsModel{
		prepareCmd: prepareCmd,
		styles:     styles,
	}
}

func (m *SettingsModel) Activate(profileName string) {
	m.profileName = profileName
	m.loadPrepareCommandValue()
	m.focused = fieldPrepareCmd
	m.editing = false
	m.confirmDelete = false
	m.prepareCmd.Blur()
}

// loadPrepareCommandValue reads the current prepare-command from config and sets the input value.
func (m *SettingsModel) loadPrepareCommandValue() {
	current := config.GetPrepareCommand()
	if current != nil {
		m.prepareCmd.SetValue(*current)
	} else {
		m.prepareCmd.SetValue("")
	}
}

func (m *SettingsModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		if m.confirmDelete {
			return m.handleConfirmDeleteKey(key)
		}

		if m.editing {
			return m.handleEditingKey(key)
		}

		return m.handleNavKey(key)
	}
	return nil
}

func (m *SettingsModel) handleConfirmDeleteKey(key string) tea.Cmd {
	switch key {
	case "y":
		return m.deleteProfile()
	case "n", "esc":
		m.confirmDelete = false
	}
	return nil
}

func (m *SettingsModel) handleEditingKey(key string) tea.Cmd {
	switch key {
	case "esc":
		m.editing = false
		m.prepareCmd.Blur()
		m.loadPrepareCommandValue()
		return nil
	case "enter":
		return m.save()
	default:
		m.prepareCmd.HandleKey(key)
		return nil
	}
}

func (m *SettingsModel) handleNavKey(key string) tea.Cmd {
	switch key {
	case "j", "down":
		m.focused = (m.focused + 1) % settingsFieldCount
		return nil
	case "k", "up":
		m.focused = (m.focused - 1 + settingsFieldCount) % settingsFieldCount
		return nil
	case "enter":
		switch m.focused {
		case fieldPrepareCmd:
			m.editing = true
			m.prepareCmd.Focus()
			return nil
		case fieldAddProfile:
			return func() tea.Msg { return showProfileRegistrationMsg{fromSettings: true} }
		case fieldDeleteProfile:
			m.confirmDelete = true
			return nil
		}
	case "esc", "q":
		return func() tea.Msg { return showWorkspaceListMsg{} }
	}
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
	m.editing = false
	m.prepareCmd.Blur()
	return func() tea.Msg { return notifyMsg{text: "Settings saved", isError: false} }
}

func (m *SettingsModel) deleteProfile() tea.Cmd {
	active := config.GetActiveProfile()
	if active == nil {
		return nil
	}
	err := config.UnregisterProfile(active.RootPath)
	if err != nil {
		return func() tea.Msg {
			return notifyMsg{text: "Failed to delete profile: " + err.Error(), isError: true}
		}
	}
	m.confirmDelete = false
	remaining := config.GetProfiles()
	if len(remaining) == 0 {
		return tea.Quit
	}
	config.SetActiveProfile(&remaining[0])
	return func() tea.Msg { return profileActivatedMsg{} }
}

// renderFieldBar returns the vertical bar and styled label for a settings field.
func (m *SettingsModel) renderFieldBar(field settingsField, text string) (bar string, label string) {
	if m.focused == field {
		bar = lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
		label = m.styles.FieldLabelFocused.Render(text)
	} else {
		bar = m.styles.Muted.Render("\u2502") + " "
		label = m.styles.FieldLabelBlurred.Render(text)
	}
	return bar, label
}

func (m *SettingsModel) View() string {
	var b strings.Builder

	// Configuration section
	b.WriteString("\n")
	b.WriteString("  " + m.styles.Title.Render("Configuration") + "\n")

	// Prepare Command
	bar, label := m.renderFieldBar(fieldPrepareCmd, "Prepare Command")
	b.WriteString("  " + bar + label + "\n")
	if m.editing {
		b.WriteString("  " + bar + m.prepareCmd.View() + "\n")
	} else {
		value := m.prepareCmd.Value
		if value == "" {
			b.WriteString("  " + bar + m.styles.Muted.Render(m.prepareCmd.Placeholder) + "\n")
		} else {
			b.WriteString("  " + bar + value + "\n")
		}
	}

	b.WriteString("\n")

	// Profiles section
	b.WriteString("  " + m.styles.Title.Render("Profiles") + "\n")

	// Add Profile
	bar, label = m.renderFieldBar(fieldAddProfile, "+ Add Profile")
	b.WriteString("  " + bar + label + "\n")

	// Delete Profile
	if m.confirmDelete && m.focused == fieldDeleteProfile {
		bar, _ = m.renderFieldBar(fieldDeleteProfile, "")
		profiles := config.GetProfiles()
		if len(profiles) <= 1 {
			b.WriteString("  " + bar + m.styles.FieldLabelFocused.Render(
				fmt.Sprintf("Delete profile %q? This is the last profile; Devora will exit. (y/n)", m.profileName),
			) + "\n")
		} else {
			b.WriteString("  " + bar + m.styles.FieldLabelFocused.Render(
				fmt.Sprintf("Delete profile %q? This only removes it from Devora. (y/n)", m.profileName),
			) + "\n")
		}
	} else {
		bar, label = m.renderFieldBar(fieldDeleteProfile, fmt.Sprintf("\u2715 Delete %q", m.profileName))
		b.WriteString("  " + bar + label + "\n")
	}

	return b.String()
}

func (m *SettingsModel) SetSize(w, h int) {
	m.width = w
}

func (m SettingsModel) ActionBindings() []components.KeyBinding {
	if m.confirmDelete {
		return []components.KeyBinding{
			{Key: "y", Desc: "Confirm"},
			{Key: "n/esc", Desc: "Cancel"},
		}
	}
	if m.editing {
		return []components.KeyBinding{
			{Key: "enter", Desc: "Save"},
			{Key: "esc", Desc: "Cancel"},
		}
	}
	return []components.KeyBinding{
		{Key: "enter", Desc: "Select"},
		{Key: "j/k", Desc: "Navigate"},
		{Key: "esc/q", Desc: "Back"},
	}
}

func (m SettingsModel) borderTitle() string {
	if m.profileName != "" {
		return fmt.Sprintf("Settings . %s", m.profileName)
	}
	return "Settings"
}
