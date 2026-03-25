package tui

import (
	"os"
	"path/filepath"
	"strings"
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

func TestRegister_ExpandsTildePath(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("could not get home dir: %v", err)
	}

	// Create a temp dir under home so we can reference it via ~/
	tmpDir, err := os.MkdirTemp(home, ".debi-test-*")
	if err != nil {
		t.Fatalf("could not create temp dir under home: %v", err)
	}
	t.Cleanup(func() { os.RemoveAll(tmpDir) })

	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	relPath, _ := filepath.Rel(home, tmpDir)
	tildePath := "~/" + filepath.Join(relPath, "tilde-profile")

	m := &ProfileRegModel{}
	m.pathInput.SetValue(tildePath)
	m.nameInput.SetValue("tilde-test")

	cmd := m.register()
	if cmd == nil {
		t.Fatalf("expected non-nil cmd, got nil. errMsg: %s", m.errMsg)
	}

	active := config.GetActiveProfile()
	if active == nil {
		t.Fatal("expected active profile to be set")
	}
	if strings.Contains(active.RootPath, "~") {
		t.Fatalf("expected tilde to be expanded in RootPath, got %q", active.RootPath)
	}

	expectedPath := filepath.Join(home, relPath, "tilde-profile")
	if active.RootPath != expectedPath {
		t.Fatalf("expected RootPath %q, got %q", expectedPath, active.RootPath)
	}
}

func TestRegister_PathIsFileShowsError(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	// Create a regular file (not a directory)
	filePath := filepath.Join(tmpDir, "not-a-dir")
	if err := os.WriteFile(filePath, []byte("hello"), 0o644); err != nil {
		t.Fatalf("failed to create file: %v", err)
	}

	m := &ProfileRegModel{}
	m.pathInput.SetValue(filePath)
	m.nameInput.SetValue("test")

	cmd := m.register()
	if cmd != nil {
		t.Fatal("expected nil cmd when path is a file")
	}
	if m.errMsg == "" {
		t.Fatal("expected error message when path is a file")
	}
	if m.errMsg != "Path exists but is not a directory" {
		t.Fatalf("expected 'Path exists but is not a directory', got %q", m.errMsg)
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

func TestProfileReg_ViewShowsExplanatoryLabel(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewProfileRegModel(&styles, true)

	view := m.View()

	expectedPhrases := []string{
		"config.json",
		"repos/",
		"workspaces/",
	}
	for _, phrase := range expectedPhrases {
		if !strings.Contains(view, phrase) {
			t.Errorf("expected View() to contain %q, but it did not.\nView output:\n%s", phrase, view)
		}
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
