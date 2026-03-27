package tui

import (
	"path/filepath"
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/config"
)

func setupSettingsConfig(t *testing.T) (config.Profile, string) {
	t.Helper()
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

	return profile, tmpDir
}

func newSettingsModel(t *testing.T) SettingsModel {
	t.Helper()
	styles := NewStyles(ThemePalette{})
	return NewSettingsModel(&styles)
}

func settingsKeyMsg(code rune) tea.KeyPressMsg {
	return tea.KeyPressMsg(tea.Key{Code: code})
}

func TestSettings_SaveEmitsNotificationOnly(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")

	cmd := m.save()
	if cmd == nil {
		t.Fatal("expected non-nil command from save()")
	}

	msg := cmd()

	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if notify.isError {
		t.Fatal("expected non-error notification")
	}
	if notify.text != "Settings saved" {
		t.Fatalf("expected 'Settings saved', got %q", notify.text)
	}
}

func TestSettings_SaveErrorEmitsErrorNotification(t *testing.T) {
	tmpDir := t.TempDir()
	config.SetConfigPathForTesting(filepath.Join(tmpDir, "nonexistent", "config.json"))
	t.Cleanup(func() {
		config.ResetForTesting()
	})

	m := newSettingsModel(t)

	cmd := m.save()
	if cmd == nil {
		t.Fatal("expected non-nil command from save()")
	}

	msg := cmd()

	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg on error, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
}

func TestSettings_EscInNavModeGoesBack(t *testing.T) {
	m := newSettingsModel(t)

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
	m := newSettingsModel(t)

	cmd := m.Update(settingsKeyMsg('q'))

	if cmd == nil {
		t.Fatal("expected a command for back, got nil")
	}
	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestSettings_EscInEditingModeExitsEditing(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")
	m.editing = true
	m.prepareCmd.Focus()

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if m.editing {
		t.Fatal("expected editing to be false after esc")
	}
	if m.prepareCmd.Focused {
		t.Fatal("expected prepareCmd to be blurred after esc in editing mode")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd (exit editing only, not navigate)")
	}
}

func TestSettings_QInEditingModeTypesLetter(t *testing.T) {
	m := newSettingsModel(t)
	m.editing = true
	m.prepareCmd.Focus()

	m.Update(settingsKeyMsg('q'))

	if m.prepareCmd.Value != "q" {
		t.Fatalf("expected prepareCmd to contain 'q', got %q", m.prepareCmd.Value)
	}
	if !m.editing {
		t.Fatal("expected editing to remain true after typing q")
	}
}

func TestSettings_EnterInEditingModeSaves(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")
	m.editing = true
	m.prepareCmd.Focus()
	m.prepareCmd.HandleKey("m")
	m.prepareCmd.HandleKey("a")
	m.prepareCmd.HandleKey("k")
	m.prepareCmd.HandleKey("e")

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if m.editing {
		t.Fatal("expected editing to be false after enter (save)")
	}
	if cmd == nil {
		t.Fatal("expected non-nil cmd from save")
	}
	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if notify.isError {
		t.Fatal("expected non-error notification from save")
	}
}

func TestSettings_JKNavigatesBetweenFields(t *testing.T) {
	m := newSettingsModel(t)

	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected initial focus on fieldPrepareCmd, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldAddProfile {
		t.Fatalf("expected fieldAddProfile after j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldDeleteProfile {
		t.Fatalf("expected fieldDeleteProfile after second j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected fieldPrepareCmd after third j (wrap), got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != fieldDeleteProfile {
		t.Fatalf("expected fieldDeleteProfile after k (wrap back), got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != fieldAddProfile {
		t.Fatalf("expected fieldAddProfile after second k, got %d", m.focused)
	}
}

func TestSettings_EnterOnPrepareCmdStartsEditing(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = fieldPrepareCmd

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if !m.editing {
		t.Fatal("expected editing to be true after enter on fieldPrepareCmd")
	}
	if !m.prepareCmd.Focused {
		t.Fatal("expected prepareCmd to be focused after enter on fieldPrepareCmd")
	}
}

func TestSettings_EnterOnAddProfileEmitsMessage(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = fieldAddProfile

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if cmd == nil {
		t.Fatal("expected non-nil cmd for add profile")
	}
	msg := cmd()
	regMsg, ok := msg.(showProfileRegistrationMsg)
	if !ok {
		t.Fatalf("expected showProfileRegistrationMsg, got %T", msg)
	}
	if !regMsg.fromSettings {
		t.Fatal("expected fromSettings to be true")
	}
}

func TestSettings_EnterOnDeleteProfileStartsConfirmation(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = fieldDeleteProfile

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if !m.confirmDelete {
		t.Fatal("expected confirmDelete to be true after enter on fieldDeleteProfile")
	}
}

func TestSettings_YInConfirmModeDeletesProfile(t *testing.T) {
	_, tmpDir := setupSettingsConfig(t)

	// Register a second profile so deletion doesn't quit the app
	profile2, err := config.RegisterProfile(filepath.Join(tmpDir, "profile2"), "test2")
	if err != nil {
		t.Fatalf("failed to register second profile: %v", err)
	}
	_ = profile2

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDeleteProfile
	m.confirmDelete = true

	cmd := m.Update(settingsKeyMsg('y'))

	if cmd == nil {
		t.Fatal("expected non-nil cmd from delete confirmation")
	}
	msg := cmd()
	if _, ok := msg.(profileActivatedMsg); !ok {
		t.Fatalf("expected profileActivatedMsg after delete, got %T", msg)
	}
	if m.confirmDelete {
		t.Fatal("expected confirmDelete to be false after deletion")
	}
}

func TestSettings_NInConfirmModeCancels(t *testing.T) {
	m := newSettingsModel(t)
	m.confirmDelete = true

	m.Update(settingsKeyMsg('n'))

	if m.confirmDelete {
		t.Fatal("expected confirmDelete to be false after n")
	}
}

func TestSettings_EscInConfirmModeCancels(t *testing.T) {
	m := newSettingsModel(t)
	m.confirmDelete = true

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if m.confirmDelete {
		t.Fatal("expected confirmDelete to be false after esc")
	}
}

func TestSettings_ActivateResetsState(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.focused = fieldDeleteProfile
	m.editing = true
	m.confirmDelete = true
	m.prepareCmd.Focus()

	m.Activate("test")

	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected focused to be fieldPrepareCmd after Activate, got %d", m.focused)
	}
	if m.editing {
		t.Fatal("expected editing to be false after Activate")
	}
	if m.confirmDelete {
		t.Fatal("expected confirmDelete to be false after Activate")
	}
	if m.prepareCmd.Focused {
		t.Fatal("expected prepareCmd to be blurred after Activate")
	}
}

func TestSettings_ActionBindings_NavMode(t *testing.T) {
	m := newSettingsModel(t)

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "enter", "Select") {
		t.Fatal("expected 'enter Select' binding in nav mode")
	}
	if !hasBinding(bindings, "j/k", "Navigate") {
		t.Fatal("expected 'j/k Navigate' binding in nav mode")
	}
	if !hasBinding(bindings, "esc/q", "Back") {
		t.Fatal("expected 'esc/q Back' binding in nav mode")
	}
}

func TestSettings_ActionBindings_EditingMode(t *testing.T) {
	m := newSettingsModel(t)
	m.editing = true

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "enter", "Save") {
		t.Fatal("expected 'enter Save' binding in editing mode")
	}
	if !hasBinding(bindings, "esc", "Cancel") {
		t.Fatal("expected 'esc Cancel' binding in editing mode")
	}
}

func TestSettings_ActionBindings_ConfirmMode(t *testing.T) {
	m := newSettingsModel(t)
	m.confirmDelete = true

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "y", "Confirm") {
		t.Fatal("expected 'y Confirm' binding in confirm mode")
	}
	if !hasBinding(bindings, "n/esc", "Cancel") {
		t.Fatal("expected 'n/esc Cancel' binding in confirm mode")
	}
}

func TestSettings_CtrlCNotHandledAtPageLevel(t *testing.T) {
	m := newSettingsModel(t)

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl}))

	if cmd != nil {
		t.Fatal("expected nil cmd for ctrl+c at page level (AppModel handles it)")
	}
}

func TestSettings_DeleteLastProfileQuitsApp(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDeleteProfile
	m.confirmDelete = true

	cmd := m.Update(settingsKeyMsg('y'))

	if cmd == nil {
		t.Fatal("expected non-nil cmd when deleting last profile")
	}

	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Fatalf("expected tea.QuitMsg when deleting last profile, got %T", msg)
	}
}
