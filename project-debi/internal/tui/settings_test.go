package tui

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	tea "charm.land/bubbletea/v2"
	"devora/internal/config"
)

func readJSONFile(t *testing.T, path string) map[string]any {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read %s: %v", path, err)
	}
	var data map[string]any
	if err := json.Unmarshal(b, &data); err != nil {
		t.Fatalf("failed to parse %s: %v", path, err)
	}
	return data
}

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
	m.focused = fieldPrepareCmd

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
	m.focused = fieldPrepareCmd
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
	m.focused = fieldPrepareCmd
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
	m.focused = fieldPrepareCmd
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
	// No explicit repos => fields: defaultAppGlobal(0), defaultAppProfile(1), prepareCmd(2), addRepo(3), viewChangelog(4), addProfile(5), deleteProfile(6)

	if m.focused != fieldDefaultAppGlobal {
		t.Fatalf("expected initial focus on fieldDefaultAppGlobal, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldDefaultAppProfile {
		t.Fatalf("expected fieldDefaultAppProfile after j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected fieldPrepareCmd after second j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldAddRepo {
		t.Fatalf("expected fieldAddRepo after third j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != m.viewChangelogField() {
		t.Fatalf("expected viewChangelogField after fourth j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != m.addProfileField() {
		t.Fatalf("expected addProfileField after fifth j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != m.deleteProfileField() {
		t.Fatalf("expected deleteProfileField after sixth j, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j'))
	if m.focused != fieldDefaultAppGlobal {
		t.Fatalf("expected fieldDefaultAppGlobal after seventh j (wrap), got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != m.deleteProfileField() {
		t.Fatalf("expected deleteProfileField after k (wrap back), got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != m.addProfileField() {
		t.Fatalf("expected addProfileField after second k, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('k'))
	if m.focused != m.viewChangelogField() {
		t.Fatalf("expected viewChangelogField after third k, got %d", m.focused)
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

func TestSettings_EnterOnViewChangelogEmitsMessage(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = m.viewChangelogField()

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if cmd == nil {
		t.Fatal("expected non-nil cmd for view changelog")
	}
	msg := cmd()
	if _, ok := msg.(openChangelogMsg); !ok {
		t.Fatalf("expected openChangelogMsg, got %T", msg)
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

	if m.focused != fieldDefaultAppGlobal {
		t.Fatalf("expected focused to be fieldDefaultAppGlobal after Activate, got %d", m.focused)
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
	// Fields: defaultAppGlobal(0), defaultAppProfile(1), prepareCmd(2), addRepo(3), removeRepo-a(4), removeRepo-b(5), viewChangelog(6), addProfile(7), deleteProfile(8)

	if m.fieldCount() != 9 {
		t.Fatalf("expected fieldCount 9, got %d", m.fieldCount())
	}

	m.focused = fieldDefaultAppGlobal
	m.Update(settingsKeyMsg('j')) // -> defaultAppProfile (1)
	if m.focused != fieldDefaultAppProfile {
		t.Fatalf("expected fieldDefaultAppProfile, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> prepareCmd (2)
	if m.focused != fieldPrepareCmd {
		t.Fatalf("expected fieldPrepareCmd, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> addRepo (3)
	if m.focused != fieldAddRepo {
		t.Fatalf("expected fieldAddRepo, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> removeRepo-a (4)
	if m.focused != m.removeRepoBaseField() {
		t.Fatalf("expected removeRepoBaseField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> removeRepo-b (5)
	if m.focused != m.removeRepoBaseField()+1 {
		t.Fatalf("expected removeRepoBaseField+1, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> viewChangelog (6)
	if m.focused != m.viewChangelogField() {
		t.Fatalf("expected viewChangelogField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> addProfile (7)
	if m.focused != m.addProfileField() {
		t.Fatalf("expected addProfileField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> deleteProfile (8)
	if m.focused != m.deleteProfileField() {
		t.Fatalf("expected deleteProfileField, got %d", m.focused)
	}

	m.Update(settingsKeyMsg('j')) // -> defaultAppGlobal (0, wrap)
	if m.focused != fieldDefaultAppGlobal {
		t.Fatalf("expected fieldDefaultAppGlobal after wrap, got %d", m.focused)
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
	// No repos: defaultAppGlobal + defaultAppProfile + prepareCmd + addRepo + viewChangelog + addProfile + deleteProfile = 7
	if m.fieldCount() != 7 {
		t.Fatalf("expected fieldCount 7 with no repos, got %d", m.fieldCount())
	}
}

func TestSettings_IsRemoveRepoField(t *testing.T) {
	m := newSettingsModel(t)
	m.explicitRepos = []config.ExplicitRepoEntry{
		{Name: "a", Path: "/a"},
		{Name: "b", Path: "/b"},
	}

	if m.isRemoveRepoField(fieldDefaultAppGlobal) {
		t.Fatal("fieldDefaultAppGlobal should not be a remove repo field")
	}
	if m.isRemoveRepoField(fieldDefaultAppProfile) {
		t.Fatal("fieldDefaultAppProfile should not be a remove repo field")
	}
	if m.isRemoveRepoField(fieldPrepareCmd) {
		t.Fatal("fieldPrepareCmd should not be a remove repo field")
	}
	if m.isRemoveRepoField(fieldAddRepo) {
		t.Fatal("fieldAddRepo should not be a remove repo field")
	}
	base := m.removeRepoBaseField()
	if !m.isRemoveRepoField(base) {
		t.Fatalf("field %d (base) should be a remove repo field", base)
	}
	if !m.isRemoveRepoField(base + 1) {
		t.Fatalf("field %d (base+1) should be a remove repo field", base+1)
	}
	if m.isRemoveRepoField(m.addProfileField()) {
		t.Fatal("addProfileField should not be a remove repo field")
	}
}

// --- Default app settings tests ---

func TestSettings_EnterOnDefaultAppGlobalStartsEditing(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = fieldDefaultAppGlobal

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if !m.editing {
		t.Fatal("expected editing to be true after enter on fieldDefaultAppGlobal")
	}
	if !m.defaultAppGlobal.Focused {
		t.Fatal("expected defaultAppGlobal to be focused after enter on fieldDefaultAppGlobal")
	}
}

func TestSettings_EnterOnDefaultAppProfileStartsEditing(t *testing.T) {
	m := newSettingsModel(t)
	m.focused = fieldDefaultAppProfile

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))

	if !m.editing {
		t.Fatal("expected editing to be true after enter on fieldDefaultAppProfile")
	}
	if !m.defaultAppProfile.Focused {
		t.Fatal("expected defaultAppProfile to be focused after enter on fieldDefaultAppProfile")
	}
}

func TestSettings_SavesDefaultAppGlobal(t *testing.T) {
	_, tmpDir := setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDefaultAppGlobal
	m.editing = true
	m.defaultAppGlobal.Focus()
	m.defaultAppGlobal.HandleKey("n")
	m.defaultAppGlobal.HandleKey("v")
	m.defaultAppGlobal.HandleKey("i")
	m.defaultAppGlobal.HandleKey("m")

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
		t.Fatalf("expected non-error notification, got error: %s", notify.text)
	}

	cfg := readJSONFile(t, filepath.Join(tmpDir, "config.json"))
	terminal, ok := cfg["terminal"].(map[string]any)
	if !ok {
		t.Fatalf("expected 'terminal' object in global config, got %v", cfg["terminal"])
	}
	if terminal["default-app"] != "nvim" {
		t.Fatalf("expected 'nvim' in global terminal.default-app, got %v", terminal["default-app"])
	}
}

func TestSettings_SavesDefaultAppProfile(t *testing.T) {
	profile, _ := setupSettingsConfig(t)

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDefaultAppProfile
	m.editing = true
	m.defaultAppProfile.Focus()
	m.defaultAppProfile.HandleKey("n")
	m.defaultAppProfile.HandleKey("v")
	m.defaultAppProfile.HandleKey("i")
	m.defaultAppProfile.HandleKey("m")

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
		t.Fatalf("expected non-error notification, got error: %s", notify.text)
	}

	cfg := readJSONFile(t, filepath.Join(profile.RootPath, "config.json"))
	terminal, ok := cfg["terminal"].(map[string]any)
	if !ok {
		t.Fatalf("expected 'terminal' object in profile config, got %v", cfg["terminal"])
	}
	if terminal["default-app"] != "nvim" {
		t.Fatalf("expected 'nvim' in profile terminal.default-app, got %v", terminal["default-app"])
	}
}

func TestSettings_EmptyValueClearsProfileDefaultAppOverride(t *testing.T) {
	profile, _ := setupSettingsConfig(t)

	// Seed profile with an override
	seeded := "nvim"
	if err := config.SetDefaultTerminalAppProfile(&seeded); err != nil {
		t.Fatalf("failed to seed profile default-app: %v", err)
	}

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDefaultAppProfile
	m.editing = true
	m.defaultAppProfile.Focus()
	// Clear the value
	m.defaultAppProfile.SetValue("")

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))
	if cmd == nil {
		t.Fatal("expected non-nil cmd from save")
	}
	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if notify.isError {
		t.Fatalf("expected non-error notification, got error: %s", notify.text)
	}

	cfg := readJSONFile(t, filepath.Join(profile.RootPath, "config.json"))
	if _, exists := cfg["terminal"]; exists {
		t.Fatalf("expected 'terminal' key to be removed from profile config, got %v", cfg["terminal"])
	}
}

func TestSettings_EmptyValueClearsGlobalDefaultApp(t *testing.T) {
	_, tmpDir := setupSettingsConfig(t)

	// Seed global with a value
	seeded := "nvim"
	if err := config.SetDefaultTerminalAppGlobal(&seeded); err != nil {
		t.Fatalf("failed to seed global default-app: %v", err)
	}

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDefaultAppGlobal
	m.editing = true
	m.defaultAppGlobal.Focus()
	m.defaultAppGlobal.SetValue("")

	cmd := m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEnter}))
	if cmd == nil {
		t.Fatal("expected non-nil cmd from save")
	}
	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if notify.isError {
		t.Fatalf("expected non-error notification, got error: %s", notify.text)
	}

	cfg := readJSONFile(t, filepath.Join(tmpDir, "config.json"))
	if _, exists := cfg["terminal"]; exists {
		t.Fatalf("expected 'terminal' key to be removed from global config, got %v", cfg["terminal"])
	}
}

func TestSettings_EscRevertsDefaultAppEdit(t *testing.T) {
	setupSettingsConfig(t)

	// Seed global with an initial value
	seeded := "shell"
	if err := config.SetDefaultTerminalAppGlobal(&seeded); err != nil {
		t.Fatalf("failed to seed global default-app: %v", err)
	}

	m := newSettingsModel(t)
	m.Activate("test")
	m.focused = fieldDefaultAppGlobal
	m.editing = true
	m.defaultAppGlobal.Focus()
	// Type a different value
	m.defaultAppGlobal.SetValue("nvim")

	m.Update(tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape}))

	if m.editing {
		t.Fatal("expected editing to be false after esc")
	}
	if m.defaultAppGlobal.Value != "shell" {
		t.Fatalf("expected defaultAppGlobal value to revert to 'shell', got %q", m.defaultAppGlobal.Value)
	}
}

func TestSettings_ActivateLoadsDefaultAppValues(t *testing.T) {
	setupSettingsConfig(t)

	globalValue := "shell"
	if err := config.SetDefaultTerminalAppGlobal(&globalValue); err != nil {
		t.Fatalf("failed to seed global default-app: %v", err)
	}
	profileValue := "nvim"
	if err := config.SetDefaultTerminalAppProfile(&profileValue); err != nil {
		t.Fatalf("failed to seed profile default-app: %v", err)
	}

	m := newSettingsModel(t)
	m.Activate("test")

	if m.defaultAppGlobal.Value != "shell" {
		t.Fatalf("expected defaultAppGlobal to load 'shell', got %q", m.defaultAppGlobal.Value)
	}
	if m.defaultAppProfile.Value != "nvim" {
		t.Fatalf("expected defaultAppProfile to load 'nvim', got %q", m.defaultAppProfile.Value)
	}
}
