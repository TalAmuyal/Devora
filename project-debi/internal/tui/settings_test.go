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
