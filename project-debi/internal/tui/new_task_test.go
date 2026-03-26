package tui

import (
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/tui/components"
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

// Two-stage esc navigation tests

func TestNewTask_EscOnTextFieldUnfocuses(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldTaskName
	m.updateFocus()

	// In insert mode (default), esc should unfocus (set navMode = true)
	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if !m.navMode {
		t.Fatal("expected navMode to be true after esc on text field")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd (unfocus only, not back)")
	}
}

func TestNewTask_QOnTextFieldTypesLetter(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldTaskName
	m.updateFocus()

	// In insert mode, q should type the letter
	m.Update(tea.KeyPressMsg(tea.Key{Code: 'q'}))

	if m.taskName.Value != "q" {
		t.Fatalf("expected taskName to contain 'q', got %q", m.taskName.Value)
	}
	if m.navMode {
		t.Fatal("expected navMode to remain false after typing q")
	}
}

func TestNewTask_EscInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
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

func TestNewTask_QInNavModeGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
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

func TestNewTask_EscOnNonTextFieldGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldRepos
	m.navMode = false
	m.updateFocus()

	// On non-text field (repos), esc should go back directly
	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestNewTask_QOnNonTextFieldGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldRepos
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

func TestNewTask_EscOnSubmitGoesBack(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldSubmit
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

func TestNewTask_TabToTextFieldExitsNavMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldRepos
	m.navMode = true

	// Tab from repos -> taskName should exit nav mode
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.focused != fieldTaskName {
		t.Fatalf("expected focus on fieldTaskName, got %d", m.focused)
	}
	if m.navMode {
		t.Fatal("expected navMode to be false after tab to text field")
	}
}

func TestNewTask_TabAwayFromTextFieldKeepsNavMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldTaskName
	m.navMode = true

	// Tab from taskName -> submit should keep navMode true
	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))

	if m.focused != fieldSubmit {
		t.Fatalf("expected focus on fieldSubmit, got %d", m.focused)
	}
	if !m.navMode {
		t.Fatal("expected navMode to remain true after tab to non-text field")
	}
}

func TestNewTask_ResetClearsNavMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.navMode = true

	m.Reset()

	if m.navMode {
		t.Fatal("expected navMode to be false after Reset()")
	}
}

func TestNewTask_CtrlCNotHandledAtPageLevel(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl}))

	if cmd != nil {
		t.Fatal("expected nil cmd for ctrl+c at page level (AppModel handles it)")
	}
}

func TestNewTask_ActionBindings_InsertModeOnTextField(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldTaskName
	m.navMode = false

	bindings := m.ActionBindings()

	hasEscUnfocus := false
	for _, b := range bindings {
		if b.Key == "esc" && b.Desc == "Unfocus" {
			hasEscUnfocus = true
		}
	}
	if !hasEscUnfocus {
		t.Fatal("expected 'esc Unfocus' binding in insert mode on text field")
	}
}

func TestNewTask_ActionBindings_NavMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.navMode = true

	bindings := m.ActionBindings()

	hasEscQBack := false
	for _, b := range bindings {
		if b.Key == "esc/q" && b.Desc == "Back" {
			hasEscQBack = true
		}
	}
	if !hasEscQBack {
		t.Fatal("expected 'esc/q Back' binding in nav mode")
	}
}

func TestNewTask_ActionBindings_NonTextFieldInsertMode(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)
	m.focused = fieldRepos
	m.navMode = false

	bindings := m.ActionBindings()

	hasEscQBack := false
	for _, b := range bindings {
		if b.Key == "esc/q" && b.Desc == "Back" {
			hasEscQBack = true
		}
	}
	if !hasEscQBack {
		t.Fatal("expected 'esc/q Back' binding on non-text field")
	}
}

func TestNewTask_ActionBindings_NoCtrlC(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewNewTaskModel([]string{"repo1"}, &styles)

	bindings := m.ActionBindings()

	for _, b := range bindings {
		if b.Key == "ctrl+c" {
			t.Fatal("expected no ctrl+c binding in footer (AppModel handles it)")
		}
	}
}

// Helper to check if a specific binding exists
func hasBinding(bindings []components.KeyBinding, key, desc string) bool {
	for _, b := range bindings {
		if b.Key == key && b.Desc == desc {
			return true
		}
	}
	return false
}
