package components

import (
	"strings"

	"charm.land/lipgloss/v2"
)

// CheckboxListModel is a multi-select list with checkbox rendering and vim navigation.
type CheckboxListModel struct {
	Items    []string
	Selected map[int]bool
	Cursor   int
	Height   int // visible area height
	vim      VimNav

	// Styles
	cursorStyle   lipgloss.Style
	selectedStyle lipgloss.Style
}

func NewCheckboxListModel(cursorStyle lipgloss.Style, selectedStyle lipgloss.Style) CheckboxListModel {
	return CheckboxListModel{
		Selected:      make(map[int]bool),
		cursorStyle:   cursorStyle,
		selectedStyle: selectedStyle,
	}
}

func (m *CheckboxListModel) CursorDown() {
	if len(m.Items) == 0 {
		return
	}
	m.Cursor++
	if m.Cursor >= len(m.Items) {
		m.Cursor = 0
	}
}

func (m *CheckboxListModel) CursorUp() {
	if len(m.Items) == 0 {
		return
	}
	m.Cursor--
	if m.Cursor < 0 {
		m.Cursor = len(m.Items) - 1
	}
}

func (m *CheckboxListModel) GoFirst() {
	m.Cursor = 0
}

func (m *CheckboxListModel) GoLast() {
	if len(m.Items) == 0 {
		return
	}
	m.Cursor = len(m.Items) - 1
}

// Toggle toggles the selection state of the current item.
func (m *CheckboxListModel) Toggle() {
	if len(m.Items) == 0 {
		return
	}
	if m.Selected[m.Cursor] {
		delete(m.Selected, m.Cursor)
	} else {
		m.Selected[m.Cursor] = true
	}
}

// SelectedItems returns the names of all selected items.
func (m *CheckboxListModel) SelectedItems() []string {
	var result []string
	for i, item := range m.Items {
		if m.Selected[i] {
			result = append(result, item)
		}
	}
	return result
}

// HandleKey processes keys. Returns true if consumed.
func (m *CheckboxListModel) HandleKey(key string) bool {
	switch key {
	case "space":
		m.Toggle()
		return true
	}
	return m.vim.HandleKey(key, m.GoFirst, m.GoLast, m.CursorUp, m.CursorDown)
}

// Reset clears vim nav state.
func (m *CheckboxListModel) Reset() {
	m.vim.Reset()
}

// View renders the checkbox list with diamond cursor and styled checkboxes.
func (m *CheckboxListModel) View() string {
	if len(m.Items) == 0 {
		return ""
	}

	var lines []string
	for i, item := range m.Items {
		// Cursor prefix
		prefix := "  "
		if i == m.Cursor {
			prefix = m.cursorStyle.Render("\u25c6") + " "
		}

		// Checkbox
		var checkbox string
		if m.Selected[i] {
			checkbox = "[" + m.selectedStyle.Render("x") + "]"
		} else {
			checkbox = "[ ]"
		}

		// Item text
		var itemText string
		if i == m.Cursor {
			itemText = lipgloss.NewStyle().Foreground(lipgloss.Color("")).Render(item)
		} else {
			itemText = item
		}

		lines = append(lines, prefix+checkbox+" "+itemText)
	}

	return strings.Join(lines, "\n")
}
