package tui

import (
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/tui/components"
	"devora/internal/workspace"
)

type DeleteConfirmModel struct {
	info   *WorkspaceInfo
	errMsg string
	styles *Styles
	width  int
	height int
}

func NewDeleteConfirmModel(styles *Styles) DeleteConfirmModel {
	return DeleteConfirmModel{
		styles: styles,
	}
}

func (m *DeleteConfirmModel) SetWorkspace(info *WorkspaceInfo) {
	m.info = info
	m.errMsg = ""
}

func (m *DeleteConfirmModel) SetSize(w, h int) {
	m.width = w
	m.height = h
}

func (m *DeleteConfirmModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		switch key {
		case "y":
			if m.info == nil {
				return nil
			}
			info := m.info
			return func() tea.Msg {
				err := workspace.DeleteWorkspace(info.Path)
				if err != nil {
					return deleteErrorMsg{err: err}
				}
				return deleteCompleteMsg{}
			}
		case "n", "esc", "q":
			return func() tea.Msg { return showWorkspaceListMsg{} }
		}
	}
	return nil
}

func (m *DeleteConfirmModel) View() string {
	var lines []string

	displayName := ""
	if m.info != nil {
		displayName = m.info.TaskTitle
		if displayName == "" {
			displayName = m.info.Name
		}
	}

	lines = append(lines, m.styles.Title.Render("Delete workspace \""+displayName+"\"?"))
	lines = append(lines, "")
	if m.info != nil {
		lines = append(lines, m.styles.Muted.Render("Path: "+m.info.Path))
		lines = append(lines, "")
	}
	lines = append(lines, m.styles.Muted.Render("This will remove all git worktrees and the"))
	lines = append(lines, m.styles.Muted.Render("workspace directory."))

	if m.errMsg != "" {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().Foreground(m.styles.ErrorColor).Render("Error: "+m.errMsg))
	}

	content := strings.Join(lines, "\n")

	dialogWidth := 56
	if m.width > 0 && dialogWidth > m.width-8 {
		dialogWidth = m.width - 8
	}
	dialog := m.styles.DialogBorder.Width(dialogWidth).Render(content)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, dialog)
}

func (m DeleteConfirmModel) borderTitle() string {
	return "Delete Workspace"
}

func (m DeleteConfirmModel) ActionBindings() []components.KeyBinding {
	return []components.KeyBinding{
		{Key: "y", Desc: "Yes"},
		{Key: "n", Desc: "No"},
	}
}
