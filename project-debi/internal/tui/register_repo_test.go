package tui

import (
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/style"
	"devora/internal/tui/components"
)

func TestRegisterRepo_TypingClearsError(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.errMsg = "Please enter a path"

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'x'}))

	if m.errMsg != "" {
		t.Fatalf("expected errMsg to be cleared after typing, got %q", m.errMsg)
	}
}

func TestRegisterRepo_SubmitDoesNotClearError(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	// Submit with empty path — should set an error
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if m.errMsg == "" {
		t.Fatal("expected errMsg to be set after invalid submit")
	}
}

// Two-stage esc navigation tests

func TestRegisterRepo_EscInInsertModeUnfocuses(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	// Default is insert mode (navMode = false, pathInput focused)
	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if !m.navMode {
		t.Fatal("expected navMode to be true after esc in insert mode")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd (unfocus only, not back)")
	}
}

func TestRegisterRepo_QInInsertModeTypesLetter(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if m.pathInput.Value() != "q" {
		t.Fatalf("expected pathInput to contain 'q', got %q", m.pathInput.Value())
	}
	if m.navMode {
		t.Fatal("expected navMode to remain false after typing q")
	}
}

func TestRegisterRepo_EscInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
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

func TestRegisterRepo_QInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
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

func TestRegisterRepo_TypingInNavModeExitsNavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = true
	m.pathInput.Blur()

	// Typing a character should re-focus and exit nav mode
	m.Update(tea.KeyPressMsg(tea.Key{Code: 'a'}))

	if m.navMode {
		t.Fatal("expected navMode to be false after typing a character")
	}
	if !m.pathInput.Focused {
		t.Fatal("expected pathInput to be focused after typing in nav mode")
	}
}

func TestRegisterRepo_EnterInNavModeSubmits(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = true

	// Enter should submit (and refocus happens as part of that)
	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	// With empty path, it should set an error (submit validation)
	if m.errMsg == "" {
		t.Fatal("expected errMsg to be set after submitting empty path")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd for failed validation")
	}
}

func TestRegisterRepo_ResetClearsNavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = true

	m.Reset()

	if m.navMode {
		t.Fatal("expected navMode to be false after Reset()")
	}
}

func TestRegisterRepo_CtrlCNotHandledAtPageLevel(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl}))

	if cmd != nil {
		t.Fatal("expected nil cmd for ctrl+c at page level (AppModel handles it)")
	}
}

func TestRegisterRepo_ActionBindings_InsertMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = false

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc", "Unfocus") {
		t.Fatal("expected 'esc Unfocus' binding in insert mode")
	}
}

func TestRegisterRepo_ActionBindings_NavMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = true

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc/q", "Back") {
		t.Fatal("expected 'esc/q Back' binding in nav mode")
	}
}

func TestRegisterRepo_EscInNavModeGoesBackToSettings(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = true
	m.returnToSettings = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showSettingsMsg); !ok {
		t.Fatalf("expected showSettingsMsg, got %T", msg)
	}
}

func TestRegisterRepo_QInNavModeGoesBackToSettings(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.navMode = true
	m.returnToSettings = true

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showSettingsMsg); !ok {
		t.Fatalf("expected showSettingsMsg, got %T", msg)
	}
}

func TestRegisterRepo_ActionBindings_NoCtrlC(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	bindings := m.ActionBindings()

	for _, b := range bindings {
		if b.Key == "ctrl+c" {
			t.Fatal("expected no ctrl+c binding in footer")
		}
	}
}

// Browse mode tests

func enterBrowseMode(m *RegisterRepoModel) {
	m.pathInput.HandleKey("ctrl+l")
}

func TestRegisterRepo_CtrlLEntersBrowseMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	m.Update(tea.KeyPressMsg(tea.Key{Code: 'l', Mod: tea.ModCtrl}))

	if m.pathInput.Mode() != components.PathPickerBrowseMode {
		t.Fatal("expected PathPickerBrowseMode after ctrl+l")
	}
}

func TestRegisterRepo_EscInBrowseModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	enterBrowseMode(&m)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestRegisterRepo_QInBrowseModeGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	enterBrowseMode(&m)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestRegisterRepo_EnterInBrowseModeDoesNotSubmit(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	enterBrowseMode(&m)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if m.errMsg != "" {
		t.Fatalf("expected no error (enter should not submit in browse mode), got %q", m.errMsg)
	}
	if cmd != nil {
		t.Fatal("expected nil cmd in browse mode enter")
	}
}

func TestRegisterRepo_ActionBindings_BrowseMode(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	enterBrowseMode(&m)

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "esc/q", "Back") {
		t.Fatal("expected 'esc/q Back' binding in browse mode")
	}
}

func TestRegisterRepo_ActionBindings_HasBrowseHint(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "ctrl+l", "Browse") {
		t.Fatal("expected 'ctrl+l Browse' binding")
	}
}

func TestRegisterRepo_EscInBrowseModeGoesBackToSettings(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewRegisterRepoModel(&styles)
	m.returnToSettings = true
	enterBrowseMode(&m)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showSettingsMsg); !ok {
		t.Fatalf("expected showSettingsMsg, got %T", msg)
	}
}
