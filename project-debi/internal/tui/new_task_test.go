package tui

import (
	"testing"

	tea "charm.land/bubbletea/v2"
)

func TestNewTask_TypingClearsError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.errMsg = "Please select at least one repo"

	// Type a character — should clear the error
	m.Update(tea.KeyPressMsg(tea.Key{Code: 'a'}))

	if m.errMsg != "" {
		t.Fatalf("expected errMsg to be cleared after typing, got %q", m.errMsg)
	}
}

func TestNewTask_SubmitDoesNotClearError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel(nil, &styles)
	m.focused = fieldTaskName

	// Submit with empty task name — should set an error
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if m.errMsg == "" {
		t.Fatal("expected errMsg to be set after invalid submit")
	}
}

func TestNewTask_TabDoesNotClearError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.errMsg = "some error"

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.errMsg != "some error" {
		t.Fatalf("expected errMsg to persist after tab, got %q", m.errMsg)
	}
}
