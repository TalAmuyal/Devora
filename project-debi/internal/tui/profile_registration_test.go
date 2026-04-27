package tui

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/config"
	"devora/internal/style"
	"devora/internal/tui/components"
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

func TestRegister_RejectsPathWhenParentDirDoesNotExist(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	// Path where neither the target nor its parent exist
	deepPath := filepath.Join(tmpDir, "nonexistent-parent", "new-profile")

	m := &ProfileRegModel{}
	m.pathInput.SetValue(deepPath)
	m.nameInput.SetValue("test")

	cmd := m.register()
	if cmd != nil {
		t.Fatal("expected nil cmd when parent directory does not exist")
	}
	if m.errMsg != "Parent directory does not exist" {
		t.Fatalf("expected 'Parent directory does not exist', got %q", m.errMsg)
	}
}

func TestRegister_CreatesDirectoryWhenParentExists(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	// Parent exists, target does not
	profileDir := filepath.Join(tmpDir, "new-profile")

	m := &ProfileRegModel{}
	m.pathInput.SetValue(profileDir)
	m.nameInput.SetValue("test")

	cmd := m.register()
	if cmd == nil {
		t.Fatalf("expected non-nil cmd, got nil. errMsg: %s", m.errMsg)
	}
}

func TestProfileReg_TypingClearsError(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.errMsg = "Please enter a path"

	// Type a character while focused on path field
	m.Update(tea.KeyPressMsg(tea.Key{Code: 'a'}))

	if m.errMsg != "" {
		t.Fatalf("expected errMsg to be cleared after typing, got %q", m.errMsg)
	}
}

func TestProfileReg_ViewShowsExplanatoryLabel(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
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
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.errMsg = "some error"

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.errMsg != "some error" {
		t.Fatalf("expected errMsg to persist after tab, got %q", m.errMsg)
	}
}

// Two-stage esc navigation tests

func TestProfileReg_EscOnNameFieldUnfocuses(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileName
	m.updateFocus()

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if !m.navMode {
		t.Fatal("expected navMode to be true after esc on name field")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd (unfocus only, not back)")
	}
}

func TestProfileReg_EscOnPathFieldInTypeModeUnfocuses(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfilePath
	m.updateFocus()
	// PathPicker starts in type mode (insert mode)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if !m.navMode {
		t.Fatal("expected navMode to be true after esc on path field in type mode")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd (unfocus only, not back)")
	}
}

func TestProfileReg_EscOnPathFieldInBrowseModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfilePath
	m.updateFocus()

	// Switch PathPicker to browse mode by sending ctrl+l
	m.pathInput.SetValue("/tmp/")
	m.pathInput.HandleKey("ctrl+l")
	if m.pathInput.Mode() != components.PathPickerBrowseMode {
		t.Fatal("expected pathInput to be in browse mode after ctrl+l")
	}

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestProfileReg_QOnNameFieldTypesLetter(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileName
	m.updateFocus()

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if m.nameInput.Value != "q" {
		t.Fatalf("expected nameInput to contain 'q', got %q", m.nameInput.Value)
	}
	if m.navMode {
		t.Fatal("expected navMode to remain false after typing q on name field")
	}
}

func TestProfileReg_EscOnSubmitGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileSubmit
	m.navMode = false
	m.updateFocus()

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestProfileReg_QOnSubmitGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileSubmit
	m.navMode = false
	m.updateFocus()

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestProfileReg_EscInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
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

func TestProfileReg_EscInNavModeReturnsToSettings(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.returnToSettings = true
	m.navMode = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showSettingsMsg); !ok {
		t.Fatalf("expected showSettingsMsg, got %T", msg)
	}
}

func TestProfileReg_QInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
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

func TestProfileReg_EscInNavModeQuitsWhenNoBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, false) // hasBack = false (first-run)
	m.navMode = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a quit command, got nil")
	}
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Fatalf("expected tea.QuitMsg for first-run esc, got %T", msg)
	}
}

func TestProfileReg_QInNavModeQuitsWhenNoBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, false) // hasBack = false (first-run)
	m.navMode = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if cmd == nil {
		t.Fatal("expected a quit command, got nil")
	}
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Fatalf("expected tea.QuitMsg for first-run q, got %T", msg)
	}
}

func TestProfileReg_TabToNameFieldExitsNavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfilePath
	m.navMode = true

	// Tab from path -> name should exit nav mode
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.focused != fieldProfileName {
		t.Fatalf("expected focus on fieldProfileName, got %d", m.focused)
	}
	if m.navMode {
		t.Fatal("expected navMode to be false after tab to text field")
	}
}

func TestProfileReg_TabToPathFieldExitsNavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileSubmit
	m.navMode = true

	// Tab from submit -> path (wraps around) should exit nav mode
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.focused != fieldProfilePath {
		t.Fatalf("expected focus on fieldProfilePath, got %d", m.focused)
	}
	if m.navMode {
		t.Fatal("expected navMode to be false after tab to path field")
	}
}

func TestProfileReg_TabToSubmitKeepsNavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileName
	m.navMode = true

	// Tab from name -> submit should keep navMode true
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.focused != fieldProfileSubmit {
		t.Fatalf("expected focus on fieldProfileSubmit, got %d", m.focused)
	}
	if !m.navMode {
		t.Fatal("expected navMode to remain true after tab to submit field")
	}
}

func TestProfileReg_ResetClearsNavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.navMode = true

	m.Reset()

	if m.navMode {
		t.Fatal("expected navMode to be false after Reset()")
	}
}

func TestProfileReg_CtrlCNotHandledAtPageLevel(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl}))

	if cmd != nil {
		t.Fatal("expected nil cmd for ctrl+c at page level (AppModel handles it)")
	}
}

func TestProfileReg_ActionBindings_InsertModeOnNameField(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileName
	m.navMode = false

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc", "Unfocus") {
		t.Fatal("expected 'esc Unfocus' binding in insert mode on name field")
	}
}

func TestProfileReg_ActionBindings_NavModeWithBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.navMode = true

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc/q", "Back") {
		t.Fatal("expected 'esc/q Back' binding in nav mode with back")
	}
}

func TestProfileReg_ActionBindings_NavModeWithQuit(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, false) // hasBack = false (first-run)
	m.navMode = true

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc/q", "Quit") {
		t.Fatal("expected 'esc/q Quit' binding in nav mode without back (first-run)")
	}
}

func TestProfileReg_ActionBindings_NonTextFieldInsertMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)
	m.focused = fieldProfileSubmit
	m.navMode = false

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc/q", "Back") {
		t.Fatal("expected 'esc/q Back' binding on non-text field")
	}
}

func TestProfileReg_DefaultPathWhenNoProfiles(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, false) // hasBack=false means first-run, no profiles

	if m.pathInput.Value() != "~/devora" {
		t.Fatalf("expected default path '~/devora' when no profiles exist, got %q", m.pathInput.Value())
	}
}

func TestProfileReg_NoDefaultPathWhenProfilesExist(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true) // hasBack=true means profiles exist

	if m.pathInput.Value() != "" {
		t.Fatalf("expected empty path when profiles exist, got %q", m.pathInput.Value())
	}
}

func TestProfileReg_ActionBindings_NoCtrlC(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewProfileRegModel(&styles, true)

	bindings := m.ActionBindings()

	for _, b := range bindings {
		if b.Key == "ctrl+c" {
			t.Fatal("expected no ctrl+c binding in footer")
		}
	}
}
