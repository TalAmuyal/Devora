package components

import (
	"unicode"

	"charm.land/lipgloss/v2"
)

// TextInputModel is a simple single-line text input.
type TextInputModel struct {
	Value       string
	Placeholder string
	Focused     bool
	cursorPos   int

	// Styles
	focusedStyle lipgloss.Style
	blurredStyle lipgloss.Style
	cursorStyle  lipgloss.Style
}

func NewTextInputModel(focusedStyle, blurredStyle lipgloss.Style) TextInputModel {
	return TextInputModel{
		focusedStyle: focusedStyle,
		blurredStyle: blurredStyle,
		cursorStyle:  lipgloss.NewStyle().Reverse(true),
	}
}

func (m *TextInputModel) Focus() {
	m.Focused = true
}

func (m *TextInputModel) Blur() {
	m.Focused = false
}

// HandleKey processes a key press. Returns true if consumed.
func (m *TextInputModel) HandleKey(key string) bool {
	if !m.Focused {
		return false
	}

	// Bubble Tea v2 represents space as "space", normalize to literal
	if key == "space" {
		key = " "
	}

	switch key {
	case "backspace":
		if m.cursorPos > 0 {
			runes := []rune(m.Value)
			m.Value = string(runes[:m.cursorPos-1]) + string(runes[m.cursorPos:])
			m.cursorPos--
		}
		return true
	case "delete":
		runes := []rune(m.Value)
		if m.cursorPos < len(runes) {
			m.Value = string(runes[:m.cursorPos]) + string(runes[m.cursorPos+1:])
		}
		return true
	case "left":
		if m.cursorPos > 0 {
			m.cursorPos--
		}
		return true
	case "right":
		runes := []rune(m.Value)
		if m.cursorPos < len(runes) {
			m.cursorPos++
		}
		return true
	case "home", "ctrl+a":
		m.cursorPos = 0
		return true
	case "end", "ctrl+e":
		m.cursorPos = len([]rune(m.Value))
		return true
	default:
		// Insert printable characters
		keyRunes := []rune(key)
		if len(keyRunes) == 1 && unicode.IsPrint(keyRunes[0]) {
			runes := []rune(m.Value)
			newRunes := make([]rune, 0, len(runes)+1)
			newRunes = append(newRunes, runes[:m.cursorPos]...)
			newRunes = append(newRunes, keyRunes[0])
			newRunes = append(newRunes, runes[m.cursorPos:]...)
			m.Value = string(newRunes)
			m.cursorPos++
			return true
		}
		return false
	}
}

// SetValue sets the input value and moves cursor to end.
func (m *TextInputModel) SetValue(value string) {
	m.Value = value
	m.cursorPos = len([]rune(value))
}

// View renders the text input.
func (m *TextInputModel) View() string {
	var display string
	if m.Value == "" && !m.Focused {
		display = m.blurredStyle.Render(m.Placeholder)
	} else if m.Value == "" && m.Focused {
		display = m.cursorStyle.Render(" ")
	} else if m.Focused {
		runes := []rune(m.Value)
		before := string(runes[:m.cursorPos])
		if m.cursorPos < len(runes) {
			cursor := string(runes[m.cursorPos])
			after := string(runes[m.cursorPos+1:])
			display = before + m.cursorStyle.Render(cursor) + after
		} else {
			display = before + m.cursorStyle.Render(" ")
		}
	} else {
		display = m.Value
	}

	if m.Focused {
		return m.focusedStyle.Render(display)
	}
	return m.blurredStyle.Render(display)
}
