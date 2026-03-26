package tui

import (
	"fmt"
	"os"
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/config"
	"devora/internal/tui/components"
	"devora/internal/workspace"
)

// RegisterRepoModel is the form page for registering a git repo by path.
type RegisterRepoModel struct {
	pathInput components.TextInputModel
	navMode   bool
	errMsg    string
	styles    *Styles
	width     int
}

func NewRegisterRepoModel(styles *Styles) RegisterRepoModel {
	pathInput := components.NewTextInputModel(
		lipgloss.NewStyle().Foreground(styles.AccentColor),
		styles.Muted,
	)
	pathInput.Placeholder = "Enter path to git repo..."
	pathInput.Focus()

	return RegisterRepoModel{
		pathInput: pathInput,
		styles:    styles,
	}
}

// Update handles key events for the register repo page.
func (m *RegisterRepoModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		// Two-stage esc/q navigation
		if key == "esc" || key == "q" {
			return m.handleEscOrQ(key)
		}

		switch key {
		case "enter":
			return m.register()
		}

		// Any other key in nav mode: re-focus and exit nav mode
		if m.navMode {
			m.navMode = false
			m.pathInput.Focus()
		}

		m.errMsg = ""
		m.pathInput.HandleKey(key)
	}
	return nil
}

// handleEscOrQ implements two-stage esc navigation.
func (m *RegisterRepoModel) handleEscOrQ(key string) tea.Cmd {
	if m.navMode {
		return func() tea.Msg { return showWorkspaceListMsg{} }
	}

	// Insert mode
	if key == "esc" {
		m.navMode = true
		m.pathInput.Blur()
		return nil
	}
	// q types the letter
	m.errMsg = ""
	m.pathInput.HandleKey(key)
	return nil
}

func (m *RegisterRepoModel) register() tea.Cmd {
	path := strings.TrimSpace(m.pathInput.Value)

	if path == "" {
		m.errMsg = "Please enter a path"
		return nil
	}

	path = config.ExpandTilde(path)

	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		m.errMsg = "Path does not exist or is not a directory"
		return nil
	}

	if !workspace.IsGitRepo(path) {
		m.errMsg = "Path is not a git repository"
		return nil
	}

	err = config.RegisterRepo(path)
	if err != nil {
		m.errMsg = fmt.Sprintf("Error registering repo: %v", err)
		return nil
	}

	m.errMsg = ""
	return tea.Batch(
		func() tea.Msg {
			return notifyMsg{text: "Repo registered", isError: false}
		},
		func() tea.Msg { return showWorkspaceListMsg{} },
	)
}

// View renders the register repo form.
func (m *RegisterRepoModel) View() string {
	var b strings.Builder

	bar := lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
	label := m.styles.FieldLabelFocused.Render("Path")

	b.WriteString("\n")
	b.WriteString("  " + bar + label + "\n")
	b.WriteString("  " + bar + m.pathInput.View() + "\n")
	b.WriteString("\n")
	b.WriteString("  " + m.styles.Muted.Render("Enter the path to a git repository."))

	if m.errMsg != "" {
		b.WriteString("\n\n")
		b.WriteString(m.styles.NotifyError.Render("\u2717 " + m.errMsg))
	}

	return b.String()
}

// Reset clears the path input and error message.
func (m *RegisterRepoModel) Reset() {
	m.pathInput.SetValue("")
	m.pathInput.Focus()
	m.navMode = false
	m.errMsg = ""
}

// SetSize updates the width and height available.
func (m *RegisterRepoModel) SetSize(w, h int) {
	m.width = w
}

// ActionBindings returns the keybindings for the footer.
func (m RegisterRepoModel) ActionBindings() []components.KeyBinding {
	bindings := []components.KeyBinding{
		{Key: "enter", Desc: "Register"},
	}

	if m.navMode {
		bindings = append(bindings, components.KeyBinding{Key: "esc/q", Desc: "Back"})
	} else {
		bindings = append(bindings, components.KeyBinding{Key: "esc", Desc: "Unfocus"})
	}

	return bindings
}

// borderTitle returns the title displayed in the border.
func (m RegisterRepoModel) borderTitle() string {
	return "Register Repo"
}
