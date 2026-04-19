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
	fieldDefaultAppGlobal  settingsField = 0
	fieldDefaultAppProfile settingsField = 1
	fieldPrepareCmd        settingsField = 2
	fieldAddRepo           settingsField = 3
	// Fields 4 through 4+len(explicitRepos)-1 are remove-repo fields.
	// fieldAddProfile and fieldDeleteProfile are computed dynamically.
)

type SettingsModel struct {
	defaultAppGlobal  components.TextInputModel
	defaultAppProfile components.TextInputModel
	prepareCmd        components.TextInputModel
	focused           settingsField
	editing           bool
	confirmDelete     bool
	confirmRemoveIdx  int
	explicitRepos     []config.ExplicitRepoEntry
	styles            *Styles
	profileName       string
	width             int
}

func NewSettingsModel(styles *Styles) SettingsModel {
	focusedStyle := lipgloss.NewStyle().Foreground(styles.AccentColor)

	defaultAppGlobal := components.NewTextInputModel(focusedStyle, styles.Muted)
	defaultAppGlobal.Placeholder = "shell, nvim, or any command (empty = inherit)"

	defaultAppProfile := components.NewTextInputModel(focusedStyle, styles.Muted)
	defaultAppProfile.Placeholder = "shell, nvim, or any command (empty = inherit)"

	prepareCmd := components.NewTextInputModel(focusedStyle, styles.Muted)
	prepareCmd.Placeholder = "Shell command to run after worktree creation..."

	return SettingsModel{
		defaultAppGlobal:  defaultAppGlobal,
		defaultAppProfile: defaultAppProfile,
		prepareCmd:        prepareCmd,
		confirmRemoveIdx:  -1,
		styles:            styles,
	}
}

func (m *SettingsModel) fieldCount() int {
	return int(m.deleteProfileField()) + 1
}

func (m *SettingsModel) removeRepoBaseField() settingsField {
	return fieldAddRepo + 1
}

func (m *SettingsModel) viewChangelogField() settingsField {
	return m.removeRepoBaseField() + settingsField(len(m.explicitRepos))
}

func (m *SettingsModel) addProfileField() settingsField {
	return m.viewChangelogField() + 1
}

func (m *SettingsModel) deleteProfileField() settingsField {
	return m.addProfileField() + 1
}

func (m *SettingsModel) isRemoveRepoField(f settingsField) bool {
	base := m.removeRepoBaseField()
	return f >= base && f < m.viewChangelogField()
}

func (m *SettingsModel) removeRepoIndex(f settingsField) int {
	return int(f - m.removeRepoBaseField())
}

func (m *SettingsModel) Activate(profileName string) {
	m.profileName = profileName
	m.loadDefaultAppGlobalValue()
	m.loadDefaultAppProfileValue()
	m.loadPrepareCommandValue()
	m.loadExplicitRepos()
	m.focused = fieldDefaultAppGlobal
	m.editing = false
	m.confirmDelete = false
	m.confirmRemoveIdx = -1
	m.defaultAppGlobal.Blur()
	m.defaultAppProfile.Blur()
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

func (m *SettingsModel) loadDefaultAppGlobalValue() {
	if v := config.GetDefaultTerminalAppGlobalRaw(); v != nil {
		m.defaultAppGlobal.SetValue(*v)
	} else {
		m.defaultAppGlobal.SetValue("")
	}
}

func (m *SettingsModel) loadDefaultAppProfileValue() {
	if v := config.GetDefaultTerminalAppProfileRaw(); v != nil {
		m.defaultAppProfile.SetValue(*v)
	} else {
		m.defaultAppProfile.SetValue("")
	}
}

func (m *SettingsModel) currentInput() *components.TextInputModel {
	switch m.focused {
	case fieldDefaultAppGlobal:
		return &m.defaultAppGlobal
	case fieldDefaultAppProfile:
		return &m.defaultAppProfile
	case fieldPrepareCmd:
		return &m.prepareCmd
	}
	return nil
}

func (m *SettingsModel) reloadFocusedFieldValue() {
	switch m.focused {
	case fieldDefaultAppGlobal:
		m.loadDefaultAppGlobalValue()
	case fieldDefaultAppProfile:
		m.loadDefaultAppProfileValue()
	case fieldPrepareCmd:
		m.loadPrepareCommandValue()
	}
}

func (m *SettingsModel) loadExplicitRepos() {
	entries, err := config.GetExplicitRepoEntries()
	if err != nil {
		m.explicitRepos = nil
		return
	}
	m.explicitRepos = entries
}

func (m *SettingsModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		if m.confirmRemoveIdx >= 0 {
			return m.handleConfirmRemoveRepoKey(key)
		}

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

func (m *SettingsModel) handleConfirmRemoveRepoKey(key string) tea.Cmd {
	switch key {
	case "y":
		return m.removeRepo()
	case "n", "esc":
		m.confirmRemoveIdx = -1
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
	input := m.currentInput()
	switch key {
	case "esc":
		m.editing = false
		if input != nil {
			input.Blur()
		}
		m.reloadFocusedFieldValue()
		return nil
	case "enter":
		return m.save()
	default:
		if input != nil {
			input.HandleKey(key)
		}
		return nil
	}
}

func (m *SettingsModel) handleNavKey(key string) tea.Cmd {
	count := settingsField(m.fieldCount())
	switch key {
	case "j", "down":
		m.focused = (m.focused + 1) % count
		return nil
	case "k", "up":
		m.focused = (m.focused - 1 + count) % count
		return nil
	case "enter":
		if input := m.currentInput(); input != nil {
			m.editing = true
			input.Focus()
			return nil
		}
		switch {
		case m.focused == fieldAddRepo:
			return func() tea.Msg { return showRegisterRepoMsg{fromSettings: true} }
		case m.isRemoveRepoField(m.focused):
			m.confirmRemoveIdx = m.removeRepoIndex(m.focused)
			return nil
		case m.focused == m.viewChangelogField():
			return func() tea.Msg { return openChangelogMsg{} }
		case m.focused == m.addProfileField():
			return func() tea.Msg { return showProfileRegistrationMsg{fromSettings: true} }
		case m.focused == m.deleteProfileField():
			m.confirmDelete = true
			return nil
		}
	case "esc", "q":
		return func() tea.Msg { return showWorkspaceListMsg{} }
	}
	return nil
}

func (m *SettingsModel) save() tea.Cmd {
	input := m.currentInput()
	if input == nil {
		return nil
	}
	value := strings.TrimSpace(input.Value)
	var valuePtr *string
	if value != "" {
		valuePtr = &value
	}

	var err error
	switch m.focused {
	case fieldPrepareCmd:
		err = config.SetPrepareCommand(valuePtr)
	case fieldDefaultAppGlobal:
		err = config.SetDefaultTerminalAppGlobal(valuePtr)
	case fieldDefaultAppProfile:
		err = config.SetDefaultTerminalAppProfile(valuePtr)
	}
	if err != nil {
		return func() tea.Msg {
			return notifyMsg{text: fmt.Sprintf("Error saving settings: %v", err), isError: true}
		}
	}
	m.editing = false
	input.Blur()
	return func() tea.Msg { return notifyMsg{text: "Settings saved", isError: false} }
}

func (m *SettingsModel) removeRepo() tea.Cmd {
	if m.confirmRemoveIdx < 0 || m.confirmRemoveIdx >= len(m.explicitRepos) {
		m.confirmRemoveIdx = -1
		return nil
	}

	entry := m.explicitRepos[m.confirmRemoveIdx]
	err := config.UnregisterRepo(entry.Path)
	if err != nil {
		m.confirmRemoveIdx = -1
		return func() tea.Msg {
			return notifyMsg{text: "Failed to remove repo: " + err.Error(), isError: true}
		}
	}

	m.confirmRemoveIdx = -1
	m.loadExplicitRepos()

	// Clamp focus if it would be out of bounds
	maxField := settingsField(m.fieldCount() - 1)
	if m.focused > maxField {
		m.focused = maxField
	}

	return func() tea.Msg {
		return notifyMsg{text: fmt.Sprintf("Repo %q removed", entry.Name), isError: false}
	}
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

func (m *SettingsModel) renderTextInputField(b *strings.Builder, field settingsField, labelText string, input *components.TextInputModel) {
	bar, label := m.renderFieldBar(field, labelText)
	b.WriteString("  " + bar + label + "\n")
	if m.editing && m.focused == field {
		b.WriteString("  " + bar + input.View() + "\n")
	} else if input.Value == "" {
		b.WriteString("  " + bar + m.styles.Muted.Render(input.Placeholder) + "\n")
	} else {
		b.WriteString("  " + bar + input.Value + "\n")
	}
}

func (m *SettingsModel) View() string {
	var b strings.Builder

	// Configuration section
	b.WriteString("\n")
	b.WriteString("  " + m.styles.Title.Render("Configuration") + "\n")

	m.renderTextInputField(&b, fieldDefaultAppGlobal, "Default App (Global)", &m.defaultAppGlobal)
	m.renderTextInputField(&b, fieldDefaultAppProfile, "Default App (Profile)", &m.defaultAppProfile)
	m.renderTextInputField(&b, fieldPrepareCmd, "Prepare Command", &m.prepareCmd)

	b.WriteString("\n")

	// Repos section
	b.WriteString("  " + m.styles.Title.Render("Repos") + "\n")

	// Add Repo
	bar, label := m.renderFieldBar(fieldAddRepo, "+ Add Repo")
	b.WriteString("  " + bar + label + "\n")

	// Remove Repo entries
	for i, entry := range m.explicitRepos {
		field := m.removeRepoBaseField() + settingsField(i)
		if m.confirmRemoveIdx == i && m.focused == field {
			bar, _ = m.renderFieldBar(field, "")
			b.WriteString("  " + bar + m.styles.FieldLabelFocused.Render(
				fmt.Sprintf("Remove repo %q? (y/n)", entry.Name),
			) + "\n")
		} else {
			bar, label = m.renderFieldBar(field, fmt.Sprintf("\u2715 Remove %s", entry.Name))
			b.WriteString("  " + bar + label + "\n")
		}
	}

	b.WriteString("\n")

	// Info section
	b.WriteString("  " + m.styles.Title.Render("Info") + "\n")

	bar, label = m.renderFieldBar(m.viewChangelogField(), "View Changelog")
	b.WriteString("  " + bar + label + "\n")

	b.WriteString("\n")

	// Profiles section
	b.WriteString("  " + m.styles.Title.Render("Profiles") + "\n")

	// Add Profile
	addProfileField := m.addProfileField()
	bar, label = m.renderFieldBar(addProfileField, "+ Add Profile")
	b.WriteString("  " + bar + label + "\n")

	// Delete Profile
	deleteProfileField := m.deleteProfileField()
	if m.confirmDelete && m.focused == deleteProfileField {
		bar, _ = m.renderFieldBar(deleteProfileField, "")
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
		bar, label = m.renderFieldBar(deleteProfileField, fmt.Sprintf("\u2715 Delete %q", m.profileName))
		b.WriteString("  " + bar + label + "\n")
	}

	return b.String()
}

func (m *SettingsModel) SetSize(w, h int) {
	m.width = w
}

func (m SettingsModel) ActionBindings() []components.KeyBinding {
	if m.confirmRemoveIdx >= 0 || m.confirmDelete {
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
