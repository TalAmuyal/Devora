package tui

import (
	"testing"

	tea "charm.land/bubbletea/v2"
)

func TestAddRepo_TypingClearsError(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.errMsg = "No repo selected"
	m.focused = fieldAddRepoPostfix
	m.postfixInput.Focus()

	updated, _ := m.Update(tea.KeyPressMsg(tea.Key{Code: 'a'}))
	model := updated.(AddRepoModel)

	if model.errMsg != "" {
		t.Fatalf("expected errMsg to be cleared after typing, got %q", model.errMsg)
	}
}

func TestAddRepo_TabDoesNotClearError(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.errMsg = "some error"

	updated, _ := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))
	model := updated.(AddRepoModel)

	if model.errMsg != "some error" {
		t.Fatalf("expected errMsg to persist after tab, got %q", model.errMsg)
	}
}
