package tui

import (
	"testing"

	tea "charm.land/bubbletea/v2"
)

func TestRegisterRepo_TypingClearsError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.errMsg = "Please enter a path"

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'x'}))

	if m.errMsg != "" {
		t.Fatalf("expected errMsg to be cleared after typing, got %q", m.errMsg)
	}
}

func TestRegisterRepo_SubmitDoesNotClearError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	// Submit with empty path — should set an error
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if m.errMsg == "" {
		t.Fatal("expected errMsg to be set after invalid submit")
	}
}
