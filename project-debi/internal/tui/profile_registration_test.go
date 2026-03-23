package tui

import (
	"path/filepath"
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/config"
)

func TestRegister_SetsActiveProfile(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	if config.GetActiveProfile() != nil {
		t.Fatal("expected nil active profile initially")
	}

	profileDir := filepath.Join(tmpDir, "test-profile")

	m := &ProfileRegModel{}
	m.pathInput.SetValue(profileDir)
	m.nameInput.SetValue("test-profile")

	cmd := m.register()
	if cmd == nil {
		t.Fatalf("expected non-nil cmd, got nil. errMsg: %s", m.errMsg)
	}

	active := config.GetActiveProfile()
	if active == nil {
		t.Fatal("expected active profile to be set after register()")
	}
	if active.Name != "test-profile" {
		t.Fatalf("expected profile name 'test-profile', got %q", active.Name)
	}
}

func TestProfileReg_TypingClearsError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.errMsg = "Please enter a path"

	// Type a character while focused on path field
	m.Update(tea.KeyPressMsg(tea.Key{Code: 'a'}))

	if m.errMsg != "" {
		t.Fatalf("expected errMsg to be cleared after typing, got %q", m.errMsg)
	}
}

func TestProfileReg_TabDoesNotClearError(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.errMsg = "some error"

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.errMsg != "some error" {
		t.Fatalf("expected errMsg to persist after tab, got %q", m.errMsg)
	}
}
