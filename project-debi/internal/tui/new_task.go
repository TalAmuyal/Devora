package tui

import (
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/tui/components"
)

type newTaskField int

const (
	fieldRepos    newTaskField = iota
	fieldTaskName
	fieldSubmit
)

const newTaskFieldCount = 3

// NewTaskModel is the form page for creating a new task:
// repo selection (checkbox list) + task name (text input) + submit.
type NewTaskModel struct {
	repoList components.CheckboxListModel
	taskName components.TextInputModel
	focused  newTaskField
	errMsg   string
	styles   *Styles
	width    int
}

func NewNewTaskModel(repoNames []string, styles *Styles) NewTaskModel {
	repoList := components.NewCheckboxListModel(
		styles.CheckboxCursor,
		styles.CheckboxCheckedMark,
	)
	repoList.Items = repoNames

	taskName := components.NewTextInputModel(
		lipgloss.NewStyle().Foreground(styles.AccentColor),
		styles.Muted,
	)
	taskName.Placeholder = "Enter task name..."
	taskName.Focus()

	m := NewTaskModel{
		repoList: repoList,
		taskName: taskName,
		focused:  fieldRepos,
		styles:   styles,
	}
	m.updateFocus()
	return m
}

// SetRepoNames rebuilds the checkbox list with new repo names.
func (m *NewTaskModel) SetRepoNames(names []string) {
	m.repoList.Items = names
	m.repoList.Selected = make(map[int]bool)
	m.repoList.Cursor = 0
}

// Update handles key events for the new task form.
func (m *NewTaskModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		switch key {
		case "ctrl+c":
			return func() tea.Msg { return showWorkspaceListMsg{} }
		case "tab":
			m.focused = (m.focused + 1) % newTaskFieldCount
			m.updateFocus()
			return nil
		case "shift+tab":
			m.focused = (m.focused - 1 + newTaskFieldCount) % newTaskFieldCount
			m.updateFocus()
			return nil
		case "enter":
			if m.focused == fieldSubmit || m.focused == fieldTaskName {
				return m.submit()
			}
			return nil
		}

		// Delegate to focused component
		m.errMsg = ""
		switch m.focused {
		case fieldRepos:
			m.repoList.HandleKey(key)
		case fieldTaskName:
			m.taskName.HandleKey(key)
		}
	}
	return nil
}

func (m *NewTaskModel) submit() tea.Cmd {
	selectedRepos := m.repoList.SelectedItems()
	if len(selectedRepos) == 0 {
		m.errMsg = "Please select at least one repo"
		return nil
	}

	name := strings.TrimSpace(m.taskName.Value)
	if name == "" {
		m.errMsg = "Please enter a task name"
		return nil
	}

	m.errMsg = ""
	return func() tea.Msg {
		return NewTaskResult{
			RepoNames: selectedRepos,
			TaskName:  name,
		}
	}
}

func (m *NewTaskModel) updateFocus() {
	m.repoList.Reset()
	if m.focused == fieldTaskName {
		m.taskName.Focus()
	} else {
		m.taskName.Blur()
	}
}

// View renders the new task form.
func (m *NewTaskModel) View() string {
	var b strings.Builder

	// Repos field
	bar := m.styles.Muted.Render("\u2502") + " "
	label := m.styles.FieldLabelBlurred.Render("Repos")
	if m.focused == fieldRepos {
		bar = lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
		label = m.styles.FieldLabelFocused.Render("Repos")
	}
	b.WriteString("  " + bar + label + "\n")
	for _, line := range strings.Split(m.repoList.View(), "\n") {
		if line != "" {
			b.WriteString("  " + bar + line + "\n")
		}
	}

	b.WriteString("\n")

	// Task Name field
	bar = m.styles.Muted.Render("\u2502") + " "
	label = m.styles.FieldLabelBlurred.Render("Task Name")
	if m.focused == fieldTaskName {
		bar = lipgloss.NewStyle().Bold(true).Foreground(m.styles.AccentColor).Render("\u2503") + " "
		label = m.styles.FieldLabelFocused.Render("Task Name")
	}
	b.WriteString("  " + bar + label + "\n")
	b.WriteString("  " + bar + m.taskName.View() + "\n")

	b.WriteString("\n")

	// Submit
	submitLabel := "[ Start Task ]"
	if m.focused == fieldSubmit {
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

// Reset clears the form state.
func (m *NewTaskModel) Reset() {
	m.repoList.Selected = make(map[int]bool)
	m.repoList.Cursor = 0
	m.repoList.Reset()
	m.taskName.SetValue("")
	m.focused = fieldRepos
	m.errMsg = ""
	m.updateFocus()
}

// SetSize updates the width and height available.
func (m *NewTaskModel) SetSize(w, h int) {
	m.width = w
}

// ActionBindings returns the keybindings for the footer.
func (m NewTaskModel) ActionBindings() []components.KeyBinding {
	return []components.KeyBinding{
		{Key: "tab", Desc: "Next Field"},
		{Key: "space", Desc: "Toggle"},
		{Key: "enter", Desc: "Submit"},
		{Key: "ctrl+c", Desc: "Back"},
	}
}

// borderTitle returns the title displayed in the border.
func (m NewTaskModel) borderTitle() string {
	return "Start New Task"
}
