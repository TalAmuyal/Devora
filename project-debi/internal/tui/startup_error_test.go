package tui

import (
	"strings"
	"testing"

	tea "charm.land/bubbletea/v2"

	"devora/internal/style"
)

func TestStartupError_QuitKeys(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewStartupErrorModel(&styles, "some error")

	quitKeys := []struct {
		name string
		key  tea.Key
	}{
		{"q", tea.Key{Code: 'q'}},
		{"esc", tea.Key{Code: tea.KeyEscape}},
		{"ctrl+d", tea.Key{Code: 'd', Mod: tea.ModCtrl}},
	}

	for _, tc := range quitKeys {
		t.Run(tc.name, func(t *testing.T) {
			cmd := m.Update(tea.KeyPressMsg(tc.key))

			if cmd == nil {
				t.Fatalf("expected a quit command for %s, got nil", tc.name)
			}

			msg := cmd()
			if _, ok := msg.(tea.QuitMsg); !ok {
				t.Fatalf("expected tea.QuitMsg for %s, got %T", tc.name, msg)
			}
		})
	}
}

func TestStartupError_OtherKeysIgnored(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewStartupErrorModel(&styles, "some error")

	ignoredKeys := []struct {
		name string
		key  tea.Key
	}{
		{"a", tea.Key{Code: 'a'}},
		{"enter", tea.Key{Code: tea.KeyEnter}},
		{"tab", tea.Key{Code: tea.KeyTab}},
		{"space", tea.Key{Code: ' '}},
	}

	for _, tc := range ignoredKeys {
		t.Run(tc.name, func(t *testing.T) {
			cmd := m.Update(tea.KeyPressMsg(tc.key))

			if cmd != nil {
				t.Fatalf("expected nil command for %s, got non-nil", tc.name)
			}
		})
	}
}

func TestStartupError_ViewContainsMessage(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	errorMessage := "Expected PATH to contain /Applications/Devora.app/Contents/Resources/bundled-apps"
	m := NewStartupErrorModel(&styles, errorMessage)
	m.SetSize(80, 24)

	view := m.View()

	if !strings.Contains(view, errorMessage) {
		t.Fatalf("expected View() to contain the error message %q, but it did not.\nView output:\n%s", errorMessage, view)
	}
}

func TestStartupError_ViewContainsInstructions(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewStartupErrorModel(&styles, "some error")
	m.SetSize(80, 24)

	view := m.View()

	expectedPhrases := []string{
		".zprofile",
		".zshrc",
	}
	for _, phrase := range expectedPhrases {
		if !strings.Contains(view, phrase) {
			t.Errorf("expected View() to contain %q, but it did not.\nView output:\n%s", phrase, view)
		}
	}
}

func TestStartupError_ActionBindings(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewStartupErrorModel(&styles, "some error")

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "q", "Quit") {
		t.Fatal("expected 'q Quit' binding")
	}
	if !hasBinding(bindings, "esc", "Quit") {
		t.Fatal("expected 'esc Quit' binding")
	}
}

func TestStartupError_BorderTitle(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewStartupErrorModel(&styles, "some error")

	title := m.borderTitle()

	if title != "Startup Error" {
		t.Fatalf("expected borderTitle() to return %q, got %q", "Startup Error", title)
	}
}
