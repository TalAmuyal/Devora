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

// --- ctrl+c quits from any state ---

func addRepoCtrlC() tea.Msg {
	return tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl})
}

func addRepoCtrlD() tea.Msg {
	return tea.KeyPressMsg(tea.Key{Code: 'd', Mod: tea.ModCtrl})
}

func addRepoEsc() tea.Msg {
	return tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape})
}

func addRepoQ() tea.Msg {
	return tea.KeyPressMsg(tea.Key{Code: 'q'})
}

func assertQuitCmd(t *testing.T, cmd tea.Cmd) {
	t.Helper()
	if cmd == nil {
		t.Fatal("expected a quit command, got nil")
	}
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Fatalf("expected tea.QuitMsg, got %T", msg)
	}
}

func assertNoQuit(t *testing.T, cmd tea.Cmd) {
	t.Helper()
	if cmd == nil {
		return // nil is a valid no-op
	}
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); ok {
		t.Fatal("expected no quit, but got tea.QuitMsg")
	}
}

func TestAddRepo_CtrlC_QuitsFromForm(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	_, cmd := m.Update(addRepoCtrlC())
	assertQuitCmd(t, cmd)
}

func TestAddRepo_CtrlC_QuitsFromProgress(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.showProgress = true
	_, cmd := m.Update(addRepoCtrlC())
	assertQuitCmd(t, cmd)
}

func TestAddRepo_CtrlC_QuitsFromError(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.showProgress = true
	m.errMsg = "something went wrong"
	_, cmd := m.Update(addRepoCtrlC())
	assertQuitCmd(t, cmd)
}

// --- ctrl+d is no-op ---

func TestAddRepo_CtrlD_IsNoOp(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	updated, cmd := m.Update(addRepoCtrlD())
	model := updated.(AddRepoModel)

	if model.focused != fieldAddRepoList {
		t.Fatalf("expected focus unchanged, got %d", model.focused)
	}
	if cmd != nil {
		t.Fatal("expected nil command for ctrl+d no-op, got non-nil")
	}
}

// --- Two-stage esc: text input focused ---

func TestAddRepo_Esc_UnfocusesTextInput(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoPostfix
	m.postfixInput.Focus()
	// navMode starts false (insert mode)

	updated, cmd := m.Update(addRepoEsc())
	model := updated.(AddRepoModel)

	if model.postfixInput.Focused {
		t.Fatal("expected text input to be blurred after first esc")
	}
	if !model.navMode {
		t.Fatal("expected navMode to be true after first esc")
	}
	assertNoQuit(t, cmd)
}

func TestAddRepo_Esc_SecondEscQuits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoPostfix
	m.navMode = true
	m.postfixInput.Blur()

	_, cmd := m.Update(addRepoEsc())
	assertQuitCmd(t, cmd)
}

// --- q in insert mode types literal character ---

func TestAddRepo_Q_InInsertMode_TypesLiteral(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoPostfix
	m.postfixInput.Focus()
	// navMode is false (insert mode)

	updated, cmd := m.Update(addRepoQ())
	model := updated.(AddRepoModel)

	if model.postfixInput.Value != "q" {
		t.Fatalf("expected 'q' to be typed in text input, got %q", model.postfixInput.Value)
	}
	assertNoQuit(t, cmd)
}

// --- q in nav mode quits ---

func TestAddRepo_Q_InNavMode_Quits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoPostfix
	m.navMode = true
	m.postfixInput.Blur()

	_, cmd := m.Update(addRepoQ())
	assertQuitCmd(t, cmd)
}

// --- q/esc on non-text fields quit directly ---

func TestAddRepo_Esc_OnRepoList_Quits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoList
	// navMode is false, but list is non-text field

	_, cmd := m.Update(addRepoEsc())
	assertQuitCmd(t, cmd)
}

func TestAddRepo_Q_OnRepoList_Quits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoList

	_, cmd := m.Update(addRepoQ())
	assertQuitCmd(t, cmd)
}

func TestAddRepo_Esc_OnSubmitButton_Quits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoSubmit

	_, cmd := m.Update(addRepoEsc())
	assertQuitCmd(t, cmd)
}

func TestAddRepo_Q_OnSubmitButton_Quits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoSubmit

	_, cmd := m.Update(addRepoQ())
	assertQuitCmd(t, cmd)
}

// --- During progress: only ctrl+c works ---

func TestAddRepo_Progress_EscIsNoOp(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.showProgress = true
	m.errMsg = ""

	_, cmd := m.Update(addRepoEsc())
	assertNoQuit(t, cmd)
}

func TestAddRepo_Progress_QIsNoOp(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.showProgress = true
	m.errMsg = ""

	_, cmd := m.Update(addRepoQ())
	assertNoQuit(t, cmd)
}

// --- Error state: esc/q quit ---

func TestAddRepo_Error_EscQuits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.showProgress = true
	m.errMsg = "something went wrong"

	_, cmd := m.Update(addRepoEsc())
	assertQuitCmd(t, cmd)
}

func TestAddRepo_Error_QQuits(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.showProgress = true
	m.errMsg = "something went wrong"

	_, cmd := m.Update(addRepoQ())
	assertQuitCmd(t, cmd)
}

// --- Tab re-focuses text input and resets navMode ---

func TestAddRepo_Tab_ResetsNavMode(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoList
	m.navMode = true

	// Tab from list -> postfix input
	updated, _ := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab}))
	model := updated.(AddRepoModel)

	if model.focused != fieldAddRepoPostfix {
		t.Fatalf("expected focus on postfix input, got %d", model.focused)
	}
	if model.navMode {
		t.Fatal("expected navMode to be false after focusing text input via tab")
	}
}

func TestAddRepo_ShiftTab_ResetsNavMode(t *testing.T) {
	m := NewAddRepoModel(ThemePalette{}, "/tmp/ws", []string{"repo1"})
	m.focused = fieldAddRepoSubmit
	m.navMode = true

	// Shift+tab from submit -> postfix input
	updated, _ := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyTab, Mod: tea.ModShift}))
	model := updated.(AddRepoModel)

	if model.focused != fieldAddRepoPostfix {
		t.Fatalf("expected focus on postfix input, got %d", model.focused)
	}
	if model.navMode {
		t.Fatal("expected navMode to be false after focusing text input via shift+tab")
	}
}
