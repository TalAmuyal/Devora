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
	// No explicit repos => fields: prepareCmd(0), addRepo(1), addProfile(2), deleteProfile(3)

	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected initial focus on fieldPrepareCmd, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldAddRepo {
		t.Fatalf("expected fieldAddRepo after j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != m.addProfileField() {
		t.Fatalf("expected addProfileField after second j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != m.deleteProfileField() {
		t.Fatalf("expected deleteProfileField after third j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected fieldPrepareCmd after fourth j (wrap), got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != m.deleteProfileField() {
		t.Fatalf("expected deleteProfileField after k (wrap back), got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != m.addProfileField() {
		t.Fatalf("expected addProfileField after second k, got %d", m.focused)
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

func TestSettings_EnterOnAddRepoEmitsMessage(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = fieldAddRepo

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if cmd == nil {
		t.Fatal("expected non-nil cmd for add repo")
	}
	msg := cmd()
	regMsg, ok := msg.(showRegisterRepoMsg)
	if !ok {
		t.Fatalf("expected showRegisterRepoMsg, got %T", msg)
	}
	if !regMsg.fromSettings {
		t.Fatal("expected fromSettings to be true")
	}
}

func TestSettings_EnterOnAddProfileEmitsMessage(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = m.addProfileField()

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
	m.focused = m.deleteProfileField()

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if !m.confirmDelete {
		t.Fatal("expected confirmDelete to be true after enter on deleteProfileField")
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
	m.focused = m.deleteProfileField()
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
	m.focused = m.deleteProfileField()
	m.editing = true
	m.confirmDelete = true
	m.confirmRemoveIdx = 1
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
	if m.confirmRemoveIdx != -1 {
		t.Fatalf("expected confirmRemoveIdx to be -1 after Activate, got %d", m.confirmRemoveIdx)
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

func TestSettings_ActionBindings_ConfirmDeleteMode(t *testing.T) {
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

func TestSettings_ActionBindings_ConfirmRemoveRepoMode(t *testing.T) {
	m := newSettingsModel(t)
	m.confirmRemoveIdx = 0

	bindings := m.ActionBindings()

	if !hasBinding(bindings, "y", "Confirm") {
		t.Fatal("expected 'y Confirm' binding in confirm remove repo mode")
	}
	if !hasBinding(bindings, "n/esc", "Cancel") {
		t.Fatal("expected 'n/esc Cancel' binding in confirm remove repo mode")
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
	m.focused = m.deleteProfileField()
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

// Repo management tests

func TestSettings_NavigationWithExplicitRepos(t *testing.T) {
	m := newSettingsModel(t)
	m.explicitRepos = []config.ExplicitRepoEntry{
		{Name: "repo-a", Path: "/path/to/repo-a"},
		{Name: "repo-b", Path: "/path/to/repo-b"},
	}
	// Fields: prepareCmd(0), addRepo(1), removeRepo-a(2), removeRepo-b(3), addProfile(4), deleteProfile(5)

	if m.fieldCount() != 6 {
		t.Fatalf("expected fieldCount 6, got %d", m.fieldCount())
	}

	m.focused = fieldPrepareCmd
	m.Update(settingsKeyMsg('j')) // -> addRepo (1)
	if m.focused != fieldAddRepo {
		t.Fatalf("expected fieldAddRepo, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> removeRepo-a (2)
	if m.focused != m.removeRepoBaseField() {
		t.Fatalf("expected removeRepoBaseField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> removeRepo-b (3)
	if m.focused != m.removeRepoBaseField()+1 {
		t.Fatalf("expected removeRepoBaseField+1, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> addProfile (4)
	if m.focused != m.addProfileField() {
		t.Fatalf("expected addProfileField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> deleteProfile (5)
	if m.focused != m.deleteProfileField() {
		t.Fatalf("expected deleteProfileField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> prepareCmd (0, wrap)
	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected fieldPrepareCmd after wrap, got %d", m.focused)
	}
}

func TestSettings_EnterOnRemoveRepoStartsConfirmation(t *testing.T) {
	m := newSettingsModel(t)
	m.explicitRepos = []config.ExplicitRepoEntry{
		{Name: "repo-a", Path: "/path/to/repo-a"},
	}
	m.focused = m.removeRepoBaseField()

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if m.confirmRemoveIdx != 0 {
		t.Fatalf("expected confirmRemoveIdx 0, got %d", m.confirmRemoveIdx)
	}
}

func TestSettings_NInConfirmRemoveRepoCancels(t *testing.T) {
	m := newSettingsModel(t)
	m.explicitRepos = []config.ExplicitRepoEntry{
		{Name: "repo-a", Path: "/path/to/repo-a"},
	}
	m.confirmRemoveIdx = 0

	m.Update(settingsKeyMsg('n'))

	if m.confirmRemoveIdx != -1 {
		t.Fatalf("expected confirmRemoveIdx -1 after n, got %d", m.confirmRemoveIdx)
	}
}

func TestSettings_EscInConfirmRemoveRepoCancels(t *testing.T) {
	m := newSettingsModel(t)
	m.explicitRepos = []config.ExplicitRepoEntry{
		{Name: "repo-a", Path: "/path/to/repo-a"},
	}
	m.confirmRemoveIdx = 0

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if m.confirmRemoveIdx != -1 {
		t.Fatalf("expected confirmRemoveIdx -1 after esc, got %d", m.confirmRemoveIdx)
	}
}

func TestSettings_YInConfirmRemoveRepoRemovesRepo(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")

	// Register a repo that exists on disk
	repoDir := t.TempDir()
	err := config.RegisterRepo(repoDir)
	if err != nil {
		t.Fatalf("failed to register repo: %v", err)
	}
	m.loadExplicitRepos()

	if len(m.explicitRepos) == 0 {
		t.Fatal("expected at least one explicit repo after registration")
	}

	m.focused = m.removeRepoBaseField()
	m.confirmRemoveIdx = 0

	cmd := m.Update(settingsKeyMsg('y'))

	if cmd == nil {
		t.Fatal("expected non-nil cmd from remove repo confirmation")
	}
	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if notify.isError {
		t.Fatalf("expected non-error notification, got error: %s", notify.text)
	}
	if m.confirmRemoveIdx != -1 {
		t.Fatalf("expected confirmRemoveIdx -1 after removal, got %d", m.confirmRemoveIdx)
	}
}

func TestSettings_RemoveRepoClampsFocus(t *testing.T) {
	setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")

	// Register a repo that exists on disk so we can remove it
	repoDir := t.TempDir()
	err := config.RegisterRepo(repoDir)
	if err != nil {
		t.Fatalf("failed to register repo: %v", err)
	}
	m.loadExplicitRepos()

	// Focus on deleteProfile (last field)
	m.focused = m.deleteProfileField()
	m.confirmRemoveIdx = 0

	m.Update(settingsKeyMsg('y'))

	// After removing a repo, field count decreases; focus should be clamped
	maxField := settingsField(m.fieldCount() - 1)
	if m.focused > maxField {
		t.Fatalf("expected focused <= %d, got %d", maxField, m.focused)
	}
}

func TestSettings_FieldCountWithNoRepos(t *testing.T) {
	m := newSettingsModel(t)
	// No repos: prepareCmd + addRepo + addProfile + deleteProfile = 4
	if m.fieldCount() != 4 {
		t.Fatalf("expected fieldCount 4 with no repos, got %d", m.fieldCount())
	}
}

func TestSettings_IsRemoveRepoField(t *testing.T) {
	m := newSettingsModel(t)
	m.explicitRepos = []config.ExplicitRepoEntry{
		{Name: "a", Path: "/a"},
		{Name: "b", Path: "/b"},
	}

	if m.isRemoveRepoField(fieldPrepareCmd) {
		t.Fatal("fieldPrepareCmd should not be a remove repo field")
	}
	if m.isRemoveRepoField(fieldAddRepo) {
		t.Fatal("fieldAddRepo should not be a remove repo field")
	}
	if !m.isRemoveRepoField(2) {
		t.Fatal("field 2 should be a remove repo field")
	}
	if !m.isRemoveRepoField(3) {
		t.Fatal("field 3 should be a remove repo field")
	}
	if m.isRemoveRepoField(m.addProfileField()) {
		t.Fatal("addProfileField should not be a remove repo field")
	}
}
