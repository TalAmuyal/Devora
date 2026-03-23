package components

import (
	"strings"

	"charm.land/lipgloss/v2"
)

// ListItem represents an item in the list with a pre-rendered view.
type ListItem struct {
	// Content is the multi-line rendered content of this item.
	Content string
	// Value is an opaque payload associated with this item.
	Value any
}

// Height returns the number of lines this item occupies.
func (li ListItem) Height() int {
	return strings.Count(li.Content, "\n") + 1
}

// ListModel is a wrapping list with vim-style navigation,
// variable-height items, scrolling, and a left-bar selection indicator.
type ListModel struct {
	Items        []ListItem
	Cursor       int
	Height       int // visible area height in lines
	scrollOffset int // first visible line index
	vim          VimNav

	// Styles
	indicatorChar  string
	indicatorStyle lipgloss.Style
	itemPadding    int // left padding for non-selected items (to match indicator width)
}

func NewListModel(indicatorStyle lipgloss.Style) ListModel {
	return ListModel{
		indicatorChar:  "▎",
		indicatorStyle: indicatorStyle,
		itemPadding:    1, // width of "▎"
	}
}

func (m *ListModel) CursorDown() {
	if len(m.Items) == 0 {
		return
	}
	m.Cursor++
	if m.Cursor >= len(m.Items) {
		m.Cursor = 0
	}
	m.ensureCursorVisible()
}

func (m *ListModel) CursorUp() {
	if len(m.Items) == 0 {
		return
	}
	m.Cursor--
	if m.Cursor < 0 {
		m.Cursor = len(m.Items) - 1
	}
	m.ensureCursorVisible()
}

func (m *ListModel) GoFirst() {
	m.Cursor = 0
	m.scrollOffset = 0
}

func (m *ListModel) GoLast() {
	if len(m.Items) == 0 {
		return
	}
	m.Cursor = len(m.Items) - 1
	m.ensureCursorVisible()
}

// HandleKey processes vim navigation keys. Returns true if consumed.
func (m *ListModel) HandleKey(key string) bool {
	return m.vim.HandleKey(key, m.GoFirst, m.GoLast, m.CursorUp, m.CursorDown)
}

// SelectedItem returns the currently selected item, or nil if the list is empty.
func (m *ListModel) SelectedItem() *ListItem {
	if len(m.Items) == 0 || m.Cursor < 0 || m.Cursor >= len(m.Items) {
		return nil
	}
	return &m.Items[m.Cursor]
}

// Reset clears vim nav state. Call on focus change.
func (m *ListModel) Reset() {
	m.vim.Reset()
}

// SetItems replaces the list items and resets the cursor.
func (m *ListModel) SetItems(items []ListItem) {
	m.Items = items
	m.Cursor = 0
	m.scrollOffset = 0
}

// View renders the visible portion of the list.
func (m *ListModel) View() string {
	if len(m.Items) == 0 {
		return ""
	}

	height := m.Height
	if height <= 0 {
		height = 20
	}

	var lines []string
	linesUsed := 0
	padding := strings.Repeat(" ", m.itemPadding)

	for i, item := range m.Items {
		// Blank line separator between items
		if i > 0 {
			if linesUsed >= m.scrollOffset && linesUsed < m.scrollOffset+height {
				lines = append(lines, "")
			}
			linesUsed++
		}

		itemLines := strings.Split(item.Content, "\n")
		for _, line := range itemLines {
			if linesUsed >= m.scrollOffset && linesUsed < m.scrollOffset+height {
				if i == m.Cursor {
					lines = append(lines, m.indicatorStyle.Render(m.indicatorChar)+line)
				} else {
					lines = append(lines, padding+line)
				}
			}
			linesUsed++
		}
	}

	return strings.Join(lines, "\n")
}

func (m *ListModel) ensureCursorVisible() {
	if m.Height <= 0 {
		return
	}

	// Calculate the line offset of the cursor item (including separator lines)
	lineOffset := 0
	for i := 0; i < m.Cursor && i < len(m.Items); i++ {
		if i > 0 {
			lineOffset++ // blank separator line
		}
		lineOffset += m.Items[i].Height()
	}
	if m.Cursor > 0 {
		lineOffset++ // separator before the cursor item
	}

	cursorHeight := 1
	if m.Cursor < len(m.Items) {
		cursorHeight = m.Items[m.Cursor].Height()
	}

	// Scroll up if cursor is above viewport
	if lineOffset < m.scrollOffset {
		m.scrollOffset = lineOffset
	}

	// Scroll down if cursor is below viewport
	if lineOffset+cursorHeight > m.scrollOffset+m.Height {
		m.scrollOffset = lineOffset + cursorHeight - m.Height
	}
}
