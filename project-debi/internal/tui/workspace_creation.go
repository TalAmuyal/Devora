package tui

import (
	"strings"
	"time"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/tui/components"
)

var spinnerFrames = []string{"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"}

func spinnerTick() tea.Cmd {
	return func() tea.Msg {
		time.Sleep(80 * time.Millisecond)
		return spinnerTickMsg{}
	}
}

type CreationModel struct {
	status       string
	repoStates   map[string]string
	repoOrder    []string
	errMsg       string
	spinnerFrame int
	styles       *Styles
	width        int
}

func NewCreationModel(styles *Styles) CreationModel {
	return CreationModel{
		repoStates: make(map[string]string),
		styles:     styles,
	}
}

func (m *CreationModel) Reset() {
	m.status = ""
	m.repoStates = make(map[string]string)
	m.repoOrder = nil
	m.errMsg = ""
	m.spinnerFrame = 0
}

func (m *CreationModel) SetSize(w, h int) {
	m.width = w
}

func (m *CreationModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()
		if m.errMsg != "" {
			switch key {
			case "q", "esc":
				return func() tea.Msg { return showWorkspaceListMsg{} }
			}
		}
	}
	return nil
}

func (m CreationModel) borderTitle() string {
	return "Creating Workspace"
}

func (m *CreationModel) View() string {
	var lines []string

	if m.status != "" {
		lines = append(lines, "  "+m.styles.Muted.Render(m.status))
	}

	if len(m.repoOrder) > 0 {
		lines = append(lines, "")
		for _, name := range m.repoOrder {
			state := m.repoStates[name]
			nameCol := lipgloss.NewStyle().Foreground(m.styles.AccentColor).Width(18).Render(name)
			var stateText string
			if state == "ready" {
				stateText = lipgloss.NewStyle().Foreground(m.styles.SuccessColor).Render("\u2713 ready")
			} else {
				stateText = m.styles.Muted.Render(spinnerFrames[m.spinnerFrame] + " " + state)
			}
			lines = append(lines, "    "+nameCol+stateText)
		}
	}

	if m.errMsg != "" {
		lines = append(lines, "")
		lines = append(lines, m.styles.NotifyError.Render("\u2717 "+m.errMsg))
	}

	return strings.Join(lines, "\n")
}

func (m CreationModel) ActionBindings() []components.KeyBinding {
	if m.errMsg != "" {
		return []components.KeyBinding{
			{Key: "q", Desc: "Back"},
		}
	}
	return nil
}
