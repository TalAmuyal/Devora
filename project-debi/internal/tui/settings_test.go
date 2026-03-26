package tui

import (
	"path/filepath"
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/config"
)

func TestSettings_SaveNavigatesBack(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	profile, err := config.RegisterProfile(filepath.Join(tmpDir, "profile"), "test")
	if err != nil {
		t.Fatalf("failed to register profile: %v", err)
	}
	config.SetActiveProfile(&profile)

	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)

	cmd := m.save()
	if cmd == nil {
		t.Fatal("expected non-nil command from save()")
	}

	msg := cmd()
	batch, ok := msg.(tea.BatchMsg)
	if !ok {
		t.Fatalf("expected tea.BatchMsg, got %T", msg)
	}

	var hasNotify, hasNavigate bool
	for _, subCmd := range batch {
		subMsg := subCmd()
		switch subMsg.(type) {
		case notifyMsg:
			hasNotify = true
		case showWorkspaceListMsg:
			hasNavigate = true
		}
	}

	if !hasNotify {
		t.Fatal("expected batch to contain notifyMsg")
	}
	if !hasNavigate {
		t.Fatal("expected batch to contain showWorkspaceListMsg")
	}
}

func TestSettings_SaveError_DoesNotNavigate(t *testing.T) {
	// Don't set up config path — save will fail because there's no valid config
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "nonexistent", "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)

	cmd := m.save()
	if cmd == nil {
		t.Fatal("expected non-nil command from save()")
	}

	msg := cmd()

	// Should be a plain notifyMsg (error), not a batch
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg on error, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
}

// Two-stage esc navigation tests

func TestSettings_EscInInsertModeUnfocuses(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if !m.navMode {
		t.Fatal("expected navMode to be true after esc in insert mode")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd (unfocus only, not back)")
	}
}

func TestSettings_QInInsertModeTypesLetter(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if m.prepareCmd.Value != "q" {
		t.Fatalf("expected prepareCmd to contain 'q', got %q", m.prepareCmd.Value)
	}
	if m.navMode {
		t.Fatal("expected navMode to remain false after typing q")
	}
}

func TestSettings_EscInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)
	m.navMode = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestSettings_QInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)
	m.navMode = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestSettings_TypingInNavModeExitsNavMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)
	m.navMode = true
	m.prepareCmd.Blur()

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'a'}))

	if m.navMode {
		t.Fatal("expected navMode to be false after typing a character")
	}
	if !m.prepareCmd.Focused {
		t.Fatal("expected prepareCmd to be focused after typing in nav mode")
	}
}

func TestSettings_ActivateResetsNavMode(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	profile, err := config.RegisterProfile(filepath.Join(tmpDir, "profile"), "test")
	if err != nil {
		t.Fatalf("failed to register profile: %v", err)
	}
	config.SetActiveProfile(&profile)

	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)
	m.navMode = true

	m.Activate("test")

	if m.navMode {
		t.Fatal("expected navMode to be false after Activate()")
	}
}

func TestSettings_CtrlCNotHandledAtPageLevel(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl}))

	if cmd != nil {
		t.Fatal("expected nil cmd for ctrl+c at page level (AppModel handles it)")
	}
}

func TestSettings_ActionBindings_InsertMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)
	m.navMode = false

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc", "Unfocus") {
		t.Fatal("expected 'esc Unfocus' binding in insert mode")
	}
}

func TestSettings_ActionBindings_NavMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)
	m.navMode = true

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc/q", "Back") {
		t.Fatal("expected 'esc/q Back' binding in nav mode")
	}
}

func TestSettings_ActionBindings_NoCtrlC(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewSettingsModel(&styles)

	bindings := m.ActionBindings()

	for _, b := range bindings {
		if b.Key == "ctrl+c" {
			t.Fatal("expected no ctrl+c binding in footer")
		}
	}
}
