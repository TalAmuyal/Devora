package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/config"
	"devora/internal/tui/components"
)

type profileRegField int

const (
	fieldProfilePath   profileRegField = iota
	fieldProfileName
	fieldProfileSubmit
)

const profileRegFieldCount = 3

// ProfileRegModel is the form page for registering a new profile: path + name.
type ProfileRegModel struct {
	infoLabel components.MultiLineLabelModel
	pathInput components.PathPickerModel
	nameInput components.TextInputModel
	focused   profileRegField
	navMode   bool
	hasBack   bool // false on first-run (no back, escape quits)
	errMsg    string
	styles    *Styles
	width     int
}

func NewProfileRegModel(styles *Styles, hasBack bool) ProfileRegModel {
	pathInput := components.NewPathPickerModel(
		lipgloss.NewStyle().Foreground(styles.AccentColor),
		styles.Muted,
		styles.Muted,
		lipgloss.NewStyle().Bold(true).Foreground(styles.AccentColor),
		6,
	)
	pathInput.SetPlaceholder("Enter profile root path...")

	nameInput := components.NewTextInputModel(
		lipgloss.NewStyle().Foreground(styles.AccentColor),
		styles.Muted,
	)
	nameInput.Placeholder = "Profile name (for new profiles)..."

	infoLabel := components.NewMultiLineLabelModel(
		"  The chosen directory will be used as the root for Devora.",
		"  If not already present, the following structure will be created inside:",
		"    config.json   — profile-specific configurations",
		"    repos/        — A centralized place to clone your git repos here (and from which worktrees will be created)",
		"    workspaces/   — Devora-managed worktrees here (each workspaces has one or more worktrees inside it)",
		"",
	)

	m := ProfileRegModel{
		infoLabel: infoLabel,
		pathInput: pathInput,
		nameInput: nameInput,
		focused:   fieldProfilePath,
		hasBack:   hasBack,
		styles:    styles,
	}
	m.updateFocus()
	return m
}

// Update handles key events for the profile registration page.
func (m *ProfileRegModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		// Two-stage esc/q navigation
		if key == "esc" || key == "q" {
			return m.handleEscOrQ(key)
		}

		switch key {
		case "tab":
			m.focused = (m.focused + 1) % profileRegFieldCount
			m.updateFocus()
			if m.focused == fieldProfilePath || m.focused == fieldProfileName {
				m.navMode = false
			}
			return nil
		case "shift+tab":
			m.focused = (m.focused - 1 + profileRegFieldCount) % profileRegFieldCount
			m.updateFocus()
			if m.focused == fieldProfilePath || m.focused == fieldProfileName {
				m.navMode = false
			}
			return nil
		case "enter":
			if m.focused == fieldProfileSubmit || m.focused == fieldProfileName {
				return m.register()
			}
		}

		// Delegate to focused component
		m.errMsg = ""
		switch m.focused {
		case fieldProfilePath:
			m.pathInput.HandleKey(key)
		case fieldProfileName:
			m.nameInput.HandleKey(key)
		}
	}
	return nil
}

// isTextInputFocused returns true when the focused field is a text input in insert mode.
func (m *ProfileRegModel) isTextInputFocused() bool {
	if m.focused == fieldProfileName {
		return true
	}
	return m.focused == fieldProfilePath && m.pathInput.Mode() == components.PathPickerTypeMode
}

// handleEscOrQ implements two-stage esc navigation.
func (m *ProfileRegModel) handleEscOrQ(key string) tea.Cmd {
	// Nav mode: both esc and q trigger back/quit
	if m.navMode {
		return m.backOrQuit()
	}

	if m.isTextInputFocused() {
		if key == "esc" {
			m.navMode = true
			m.updateFocus()
			m.pathInput.Blur()
			m.nameInput.Blur()
			return nil
		}
		// q types the letter
		m.errMsg = ""
		switch m.focused {
		case fieldProfilePath:
			m.pathInput.HandleKey(key)
		case fieldProfileName:
			m.nameInput.HandleKey(key)
		}
		return nil
	}

	// Non-text field (submit, or path in browse mode): go back directly
	return m.backOrQuit()
}

// backOrQuit returns the appropriate back or quit command.
func (m *ProfileRegModel) backOrQuit() tea.Cmd {
	if m.hasBack {
		return func() tea.Msg { return showWorkspaceListMsg{} }
	}
	return tea.Quit
}

func (m *ProfileRegModel) register() tea.Cmd {
	path := strings.TrimSpace(m.pathInput.Value())
	name := strings.TrimSpace(m.nameInput.Value)

	if path == "" {
		m.errMsg = "Please enter a path"
		return nil
	}

	path = config.ExpandTilde(path)

	if info, err := os.Stat(path); err == nil {
		if !info.IsDir() {
			m.errMsg = "Path exists but is not a directory"
			return nil
		}
	} else {
		parentDir := filepath.Dir(path)
		parentInfo, parentErr := os.Stat(parentDir)
		if parentErr != nil || !parentInfo.IsDir() {
			m.errMsg = "Parent directory does not exist"
			return nil
		}
	}

	if !config.IsInitializedProfile(path) && name == "" {
		m.errMsg = "Please enter a profile name"
		return nil
	}

	profile, err := config.RegisterProfile(path, name)
	if err != nil {
		m.errMsg = fmt.Sprintf("Error registering profile: %v", err)
		return nil
	}

	config.SetActiveProfile(&profile)
	m.errMsg = ""
	return func() tea.Msg { return profileActivatedMsg{} }
}

func (m *ProfileRegModel) updateFocus() {
	if m.focused == fieldProfilePath {
		m.pathInput.Focus()
	} else {
		m.pathInput.Blur()
	}
	if m.focused == fieldProfileName {
		m.nameInput.Focus()
	} else {
		m.nameInput.Blur()
	}
}

// View renders the profile registration form.
func (m *ProfileRegModel) View() string {
	var b strings.Builder

	// Explanatory label
	b.WriteString("\n")
	b.WriteString(m.infoLabel.View())
	b.WriteString("\n")

	// Path field
	bar := m.styles.Muted.Render("\u2502") + " "
	label := m.styles.FieldLabelBlurred.Render("Path")
	if m.focused == fieldProfilePath {
		bar = lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
		label = m.styles.FieldLabelFocused.Render("Path")
	}
	b.WriteString("\n")
	b.WriteString("  " + bar + label + "\n")
	for _, line := range strings.Split(m.pathInput.View(), "\n") {
		b.WriteString("  " + bar + line + "\n")
	}

	b.WriteString("\n")

	// Name field
	bar = m.styles.Muted.Render("\u2502") + " "
	label = m.styles.FieldLabelBlurred.Render("Name")
	if m.focused == fieldProfileName {
		bar = lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
		label = m.styles.FieldLabelFocused.Render("Name")
	}
	b.WriteString("  " + bar + label + "\n")
	b.WriteString("  " + bar + m.nameInput.View() + "\n")

	b.WriteString("\n")

	// Submit
	submitLabel := "[ Register ]"
	if m.focused == fieldProfileSubmit {
		b.WriteString("  " + m.styles.SubmitFocused.Render(submitLabel))
	} else {
		b.WriteString("  " + m.styles.SubmitBlurred.Render(submitLabel))
	}

	if m.errMsg != "" {
		b.WriteString("\n\n")
		b.WriteString(m.styles.NotifyError.Render("\u2717 " + m.errMsg))
	}

	return b.String()
}

// Reset clears the form inputs and error message.
func (m *ProfileRegModel) Reset() {
	m.pathInput.SetValue("")
	m.nameInput.SetValue("")
	m.focused = fieldProfilePath
	m.navMode = false
	m.errMsg = ""
	m.updateFocus()
}

// SetSize updates the width and height available.
func (m *ProfileRegModel) SetSize(w, h int) {
	m.width = w
}

// ActionBindings returns the keybindings for the footer.
func (m ProfileRegModel) ActionBindings() []components.KeyBinding {
	backDesc := "Back"
	if !m.hasBack {
		backDesc = "Quit"
	}

	bindings := []components.KeyBinding{
		{Key: "tab", Desc: "Next Field"},
		{Key: "ctrl+l", Desc: "Browse"},
		{Key: "enter", Desc: "Register"},
	}

	if m.navMode || !m.isTextInputFocused() {
		bindings = append(bindings, components.KeyBinding{Key: "esc/q", Desc: backDesc})
	} else {
		bindings = append(bindings, components.KeyBinding{Key: "esc", Desc: "Unfocus"})
	}

	return bindings
}

// borderTitle returns the title displayed in the border.
func (m ProfileRegModel) borderTitle() string {
	return "Register Profile"
}
