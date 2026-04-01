package tui

import (
	"fmt"
	"strings"
	"testing"
	"time"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
)

func ctrlC() tea.Msg {
	return tea.KeyPressMsg(tea.Key{Code: 'c', Mod: tea.ModCtrl})
}

func ctrlD() tea.Msg {
	return tea.KeyPressMsg(tea.Key{Code: 'd', Mod: tea.ModCtrl})
}

func newTestAppModel() AppModel {
	palette := ThemePalette{}
	styles := NewStyles(palette)
	return AppModel{
		activePage:    PageWorkspaceList,
		styles:        styles,
		workspaceList: NewWorkspaceListModel(&styles),
		newTask:       NewNewTaskModel(nil, &styles),
		creation:      NewCreationModel(&styles),
		deleteConfirm: NewDeleteConfirmModel(&styles),
		registerRepo:  NewRegisterRepoModel(&styles),
		settings:      NewSettingsModel(&styles),
		profileReg:    NewProfileRegModel(&styles, true),
	}
}

func TestCtrlC_QuitsFromAnyPage(t *testing.T) {
	pages := []struct {
		name string
		page Page
	}{
		{"WorkspaceList", PageWorkspaceList},
		{"NewTask", PageNewTask},
		{"Creation", PageCreation},
		{"DeleteConfirm", PageDeleteConfirm},
		{"RegisterRepo", PageRegisterRepo},
		{"Settings", PageSettings},
		{"ProfileRegistration", PageProfileRegistration},
	}

	for _, tc := range pages {
		t.Run(tc.name, func(t *testing.T) {
			m := newTestAppModel()
			m.activePage = tc.page

			_, cmd := m.Update(ctrlC())

			if cmd == nil {
				t.Fatal("expected a quit command, got nil")
			}

			msg := cmd()
			if _, ok := msg.(tea.QuitMsg); !ok {
				t.Fatalf("expected tea.QuitMsg, got %T", msg)
			}
		})
	}
}

func TestCtrlD_IsNoOp(t *testing.T) {
	pages := []struct {
		name string
		page Page
	}{
		{"WorkspaceList", PageWorkspaceList},
		{"NewTask", PageNewTask},
		{"Creation", PageCreation},
	}

	for _, tc := range pages {
		t.Run(tc.name, func(t *testing.T) {
			m := newTestAppModel()
			m.activePage = tc.page

			updated, cmd := m.Update(ctrlD())
			model := updated.(AppModel)

			if model.activePage != tc.page {
				t.Fatalf("expected to stay on page %d, got page %d", tc.page, model.activePage)
			}

			if cmd != nil {
				t.Fatalf("expected nil command for ctrl+d no-op, got non-nil")
			}
		})
	}
}

// Deactivation tests

func TestDeactivateCompleteMsg_RefreshesAndNotifies(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageWorkspaceList

	updated, cmd := m.Update(deactivateCompleteMsg{})
	model := updated.(AppModel)

	if model.activePage != PageWorkspaceList {
		t.Fatalf("expected to stay on PageWorkspaceList, got page %d", model.activePage)
	}

	if cmd == nil {
		t.Fatal("expected a batch command, got nil")
	}
}

func TestDeactivateErrorMsg_ShowsNotification(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageWorkspaceList

	updated, cmd := m.Update(deactivateErrorMsg{err: fmt.Errorf("disk full")})
	_ = updated.(AppModel)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "disk full") {
		t.Fatalf("expected error text to contain 'disk full', got %q", notify.text)
	}
}

// Workspace list keybinding tests

func TestWorkspaceList_D_TriggersDeleteRequest(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{
			Path:     "/tmp/ws-1",
			Name:     "ws-1",
			Category: CategoryInactive,
			RepoGitStatuses: map[string]RepoGitStatus{
				"repo-a": {Branch: "HEAD", IsClean: true},
			},
		},
	}
	m.workspaceList.SetFilter(FilterAll)

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'D'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a command from D key, got nil")
	}

	msg := cmd()
	if _, ok := msg.(requestDeleteMsg); !ok {
		t.Fatalf("expected requestDeleteMsg from D key, got %T", msg)
	}
}

func TestWorkspaceList_d_TriggersDeactivateRequest(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{
			Path:     "/tmp/ws-1",
			Name:     "ws-1",
			Category: CategoryActiveNoSession,
			RepoGitStatuses: map[string]RepoGitStatus{
				"repo-a": {Branch: "HEAD", IsClean: true},
			},
		},
	}
	m.workspaceList.rebuildListItems()

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'd'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a command from d key, got nil")
	}

	msg := cmd()
	if _, ok := msg.(deactivateCompleteMsg); !ok {
		// Could also be deactivateErrorMsg if filesystem call fails in test
		if _, ok := msg.(deactivateErrorMsg); !ok {
			t.Fatalf("expected deactivateCompleteMsg or deactivateErrorMsg from d key, got %T", msg)
		}
	}
}

func TestWorkspaceList_Deactivate_BlockedByActiveSession(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{
			Path:     "/tmp/ws-1",
			Name:     "ws-1",
			Category: CategoryActiveWithSession,
		},
	}
	m.workspaceList.rebuildListItems()

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'd'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Cannot deactivate") {
		t.Fatalf("expected 'Cannot deactivate' in text, got %q", notify.text)
	}
}

func TestWorkspaceList_Deactivate_BlockedByDirtyRepo(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{
			Path:     "/tmp/ws-1",
			Name:     "ws-1",
			Category: CategoryActiveNoSession,
			RepoGitStatuses: map[string]RepoGitStatus{
				"repo-a": {Branch: "HEAD", IsClean: false},
			},
		},
	}
	m.workspaceList.rebuildListItems()

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'd'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !strings.Contains(notify.text, "Cannot deactivate") {
		t.Fatalf("expected 'Cannot deactivate' in text, got %q", notify.text)
	}
}

func TestWorkspaceList_Deactivate_BlockedByNonHeadBranch(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{
			Path:     "/tmp/ws-1",
			Name:     "ws-1",
			Category: CategoryActiveNoSession,
			RepoGitStatuses: map[string]RepoGitStatus{
				"repo-a": {Branch: "feature-branch", IsClean: true},
			},
		},
	}
	m.workspaceList.rebuildListItems()

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'd'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !strings.Contains(notify.text, "Cannot deactivate") {
		t.Fatalf("expected 'Cannot deactivate' in text, got %q", notify.text)
	}
}

// ActionBindings tests

func TestWorkspaceList_ActionBindings_ActiveWorkspace(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveWithSession},
	}
	m.workspaceList.rebuildListItems()

	bindings := m.workspaceList.ActionBindings()

	if !hasBinding(bindings, "d", "Deactivate") {
		t.Fatal("expected 'd Deactivate' binding for active workspace")
	}
	if hasBinding(bindings, "D", "Delete") {
		t.Fatal("should not show 'D Delete' binding for active workspace")
	}
}

func TestWorkspaceList_ActionBindings_ActiveNoSessionWorkspace(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveNoSession},
	}
	m.workspaceList.rebuildListItems()

	bindings := m.workspaceList.ActionBindings()

	if !hasBinding(bindings, "d", "Deactivate") {
		t.Fatal("expected 'd Deactivate' binding for active-no-session workspace")
	}
	if hasBinding(bindings, "D", "Delete") {
		t.Fatal("should not show 'D Delete' binding for active-no-session workspace")
	}
}

func TestWorkspaceList_ActionBindings_InactiveWorkspace(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryInactive},
	}
	m.workspaceList.SetFilter(FilterAll)

	bindings := m.workspaceList.ActionBindings()

	if hasBinding(bindings, "d", "Deactivate") {
		t.Fatal("should not show 'd Deactivate' binding for inactive workspace")
	}
	if !hasBinding(bindings, "D", "Delete") {
		t.Fatal("expected 'D Delete' binding for inactive workspace")
	}
}

func TestWorkspaceList_ActionBindings_InvalidWorkspace(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryInvalid},
	}
	m.workspaceList.SetFilter(FilterAll)

	bindings := m.workspaceList.ActionBindings()

	if hasBinding(bindings, "d", "Deactivate") {
		t.Fatal("should not show 'd Deactivate' binding for invalid workspace")
	}
	if !hasBinding(bindings, "D", "Delete") {
		t.Fatal("expected 'D Delete' binding for invalid workspace")
	}
}

func TestWorkspaceList_ActionBindings_NoSelection(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{}
	m.workspaceList.rebuildListItems()

	bindings := m.workspaceList.ActionBindings()

	if hasBinding(bindings, "d", "Deactivate") {
		t.Fatal("should not show 'd Deactivate' when no workspace selected")
	}
	if hasBinding(bindings, "D", "Delete") {
		t.Fatal("should not show 'D Delete' when no workspace selected")
	}
}

func TestWorkspaceList_ActionBindings_AlwaysHasCommonBindings(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveNoSession},
	}
	m.workspaceList.rebuildListItems()

	bindings := m.workspaceList.ActionBindings()

	if !hasBinding(bindings, "n", "New Task") {
		t.Fatal("expected 'n New Task' binding")
	}
	if !hasBinding(bindings, "r", "Refresh") {
		t.Fatal("expected 'r Refresh' binding")
	}
	if !hasBinding(bindings, "q", "Quit") {
		t.Fatal("expected 'q Quit' binding")
	}
}

// Guard tests: d key on non-active workspaces

func TestWorkspaceList_d_OnInactiveWorkspace_ReturnsNotification(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryInactive},
	}
	m.workspaceList.SetFilter(FilterAll)

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'd'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "already inactive") {
		t.Fatalf("expected 'already inactive' in text, got %q", notify.text)
	}
}

func TestWorkspaceList_d_OnInvalidWorkspace_ReturnsNotification(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryInvalid},
	}
	m.workspaceList.SetFilter(FilterAll)

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'd'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Cannot deactivate an invalid workspace") {
		t.Fatalf("expected 'Cannot deactivate an invalid workspace' in text, got %q", notify.text)
	}
}

// Guard tests: D key on active workspaces

func TestWorkspaceList_D_OnActiveWithSession_ReturnsNotification(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveWithSession},
	}
	m.workspaceList.rebuildListItems()

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'D'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Deactivate the workspace first") {
		t.Fatalf("expected 'Deactivate the workspace first' in text, got %q", notify.text)
	}
}

func TestWorkspaceList_D_OnActiveNoSession_ReturnsNotification(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveNoSession},
	}
	m.workspaceList.rebuildListItems()

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'D'})
	cmd := m.workspaceList.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Deactivate the workspace first") {
		t.Fatalf("expected 'Deactivate the workspace first' in text, got %q", notify.text)
	}
}

// Defense-in-depth: handleDeactivateRequest guards

func TestWorkspaceList_HandleDeactivateRequest_BlocksInactiveWorkspace(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryInactive},
	}
	m.workspaceList.SetFilter(FilterAll)

	cmd := m.workspaceList.handleDeactivateRequest()

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "already inactive") {
		t.Fatalf("expected 'already inactive' in text, got %q", notify.text)
	}
}

func TestWorkspaceList_HandleDeactivateRequest_BlocksInvalidWorkspace(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryInvalid},
	}
	m.workspaceList.SetFilter(FilterAll)

	cmd := m.workspaceList.handleDeactivateRequest()

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Cannot deactivate an invalid workspace") {
		t.Fatalf("expected 'Cannot deactivate an invalid workspace' in text, got %q", notify.text)
	}
}

// Defense-in-depth: handleDeleteRequest guards

func TestWorkspaceList_HandleDeleteRequest_BlocksActiveWithSession(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveWithSession},
	}
	m.workspaceList.rebuildListItems()

	cmd := m.workspaceList.handleDeleteRequest()

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Deactivate the workspace first") {
		t.Fatalf("expected 'Deactivate the workspace first' in text, got %q", notify.text)
	}
}

func TestWorkspaceList_HandleDeleteRequest_BlocksActiveNoSession(t *testing.T) {
	m := newTestAppModel()
	m.workspaceList.loading = false
	m.workspaceList.allWorkspaces = []WorkspaceInfo{
		{Path: "/tmp/ws-1", Name: "ws-1", Category: CategoryActiveNoSession},
	}
	m.workspaceList.rebuildListItems()

	cmd := m.workspaceList.handleDeleteRequest()

	if cmd == nil {
		t.Fatal("expected a notification command, got nil")
	}

	msg := cmd()
	notify, ok := msg.(notifyMsg)
	if !ok {
		t.Fatalf("expected notifyMsg, got %T", msg)
	}
	if !notify.isError {
		t.Fatal("expected error notification")
	}
	if !strings.Contains(notify.text, "Deactivate the workspace first") {
		t.Fatalf("expected 'Deactivate the workspace first' in text, got %q", notify.text)
	}
}

// Creation progress tests

func TestCreationStatusMsg_UpdatesModelAndChainsListener(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	ch := make(chan tea.Msg, 10)
	m.creationCh = ch

	updated, cmd := m.Update(creationStatusMsg{text: "Creating workspace..."})
	model := updated.(AppModel)

	if model.creation.status != "Creating workspace..." {
		t.Fatalf("expected status 'Creating workspace...', got %q", model.creation.status)
	}

	if cmd == nil {
		t.Fatal("expected a listener command, got nil")
	}

	// The command should read from the channel
	ch <- creationDoneMsg{workspacePath: "/test", sessionName: "test"}
	msg := cmd()
	if _, ok := msg.(creationDoneMsg); !ok {
		t.Fatalf("expected creationDoneMsg from listener, got %T", msg)
	}
}

func TestRepoAddedMsg_UpdatesModelAndChainsListener(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	ch := make(chan tea.Msg, 10)
	m.creationCh = ch

	updated, cmd := m.Update(repoAddedMsg{name: "my-repo"})
	model := updated.(AppModel)

	if len(model.creation.repoOrder) != 1 || model.creation.repoOrder[0] != "my-repo" {
		t.Fatalf("expected repoOrder ['my-repo'], got %v", model.creation.repoOrder)
	}
	if model.creation.repoStates["my-repo"] != "preparing..." {
		t.Fatalf("expected repoState 'preparing...', got %q", model.creation.repoStates["my-repo"])
	}
	if cmd == nil {
		t.Fatal("expected a listener command, got nil")
	}
}

func TestRepoReadyMsg_UpdatesModelAndChainsListener(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	ch := make(chan tea.Msg, 10)
	m.creationCh = ch
	m.creation.repoStates["my-repo"] = "preparing..."

	updated, cmd := m.Update(repoReadyMsg{name: "my-repo"})
	model := updated.(AppModel)

	if model.creation.repoStates["my-repo"] != "ready" {
		t.Fatalf("expected repoState 'ready', got %q", model.creation.repoStates["my-repo"])
	}
	if cmd == nil {
		t.Fatal("expected a listener command, got nil")
	}
}

func TestCreationErrorMsg_StopsListening(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	ch := make(chan tea.Msg, 10)
	m.creationCh = ch

	updated, cmd := m.Update(creationErrorMsg{err: fmt.Errorf("boom")})
	model := updated.(AppModel)

	if model.creation.errMsg != "boom" {
		t.Fatalf("expected errMsg 'boom', got %q", model.creation.errMsg)
	}
	if cmd != nil {
		t.Fatalf("expected nil command on error, got non-nil")
	}
}

func TestListenCreation_ReadsFromChannel(t *testing.T) {
	ch := make(chan tea.Msg, 1)
	ch <- creationStatusMsg{text: "hello"}

	cmd := listenCreation(ch)
	msg := cmd()

	if status, ok := msg.(creationStatusMsg); !ok || status.text != "hello" {
		t.Fatalf("expected creationStatusMsg{hello}, got %T", msg)
	}
}

func TestSpinnerTick_AdvancesFrame(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	m.creation.spinnerFrame = 0

	updated, cmd := m.Update(spinnerTickMsg{})
	model := updated.(AppModel)

	if model.creation.spinnerFrame != 1 {
		t.Fatalf("expected spinnerFrame 1, got %d", model.creation.spinnerFrame)
	}
	if cmd == nil {
		t.Fatal("expected a tick command, got nil")
	}
}

func TestSpinnerTick_WrapsAround(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	m.creation.spinnerFrame = len(spinnerFrames) - 1

	updated, _ := m.Update(spinnerTickMsg{})
	model := updated.(AppModel)

	if model.creation.spinnerFrame != 0 {
		t.Fatalf("expected spinnerFrame 0 after wrap, got %d", model.creation.spinnerFrame)
	}
}

func TestSpinnerTick_StopsOnError(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation
	m.creation.errMsg = "something failed"
	m.creation.spinnerFrame = 2

	updated, cmd := m.Update(spinnerTickMsg{})
	model := updated.(AppModel)

	// Frame should not advance
	if model.creation.spinnerFrame != 2 {
		t.Fatalf("expected spinnerFrame 2 (unchanged), got %d", model.creation.spinnerFrame)
	}
	if cmd != nil {
		t.Fatalf("expected nil command on error, got non-nil")
	}
}

func TestSpinnerTick_StopsOnWrongPage(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageWorkspaceList

	_, cmd := m.Update(spinnerTickMsg{})

	if cmd != nil {
		t.Fatalf("expected nil command on wrong page, got non-nil")
	}
}

func TestCreationView_ShowsSpinnerForPreparingRepos(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewCreationModel(&styles)
	m.repoOrder = []string{"my-repo"}
	m.repoStates = map[string]string{"my-repo": "preparing..."}
	m.spinnerFrame = 0

	view := m.View()

	expectedFrame := spinnerFrames[0]
	if !strings.Contains(view, expectedFrame) {
		t.Fatalf("expected view to contain spinner frame %q, got:\n%s", expectedFrame, view)
	}
}

func TestCreationView_NoSpinnerForReadyRepos(t *testing.T) {
	styles := NewStyles(ThemePalette{})
	m := NewCreationModel(&styles)
	m.repoOrder = []string{"my-repo"}
	m.repoStates = map[string]string{"my-repo": "ready"}
	m.spinnerFrame = 0

	view := m.View()

	if !strings.Contains(view, "\u2713 ready") {
		t.Fatalf("expected view to contain checkmark for ready repo, got:\n%s", view)
	}
	// Should NOT contain the spinner frame for the ready repo
	if strings.Contains(view, spinnerFrames[0]+" preparing") {
		t.Fatal("ready repo should not show spinner frame")
	}
}

func TestListenCreation_ClosedChannel_ReturnsNil(t *testing.T) {
	ch := make(chan tea.Msg)
	close(ch)

	cmd := listenCreation(ch)

	done := make(chan tea.Msg, 1)
	go func() {
		done <- cmd()
	}()

	select {
	case msg := <-done:
		if msg != nil {
			t.Fatalf("expected nil from closed channel, got %T", msg)
		}
	case <-time.After(time.Second):
		t.Fatal("listenCreation blocked on closed channel")
	}
}

// Sticky footer tests

// renderSizedView sends a WindowSizeMsg and returns the rendered view content.
func renderSizedView(m AppModel, width, height int) string {
	updated, _ := m.Update(tea.WindowSizeMsg{Width: width, Height: height})
	return updated.(AppModel).View().Content
}

func TestView_HeightMatchesWindowSize(t *testing.T) {
	content := renderSizedView(newTestAppModel(), 80, 24)

	if viewHeight := lipgloss.Height(content); viewHeight != 24 {
		t.Fatalf("expected view height 24, got %d", viewHeight)
	}
}

func TestView_FooterOnLastLines(t *testing.T) {
	content := renderSizedView(newTestAppModel(), 80, 24)
	lines := strings.Split(content, "\n")

	// The footer occupies the last 2 lines (separator + keybinding row).
	if len(lines) < 2 {
		t.Fatalf("expected at least 2 lines, got %d", len(lines))
	}
	separatorLine := lines[len(lines)-2]
	if !strings.Contains(separatorLine, "─") {
		t.Fatalf("expected footer separator on second-to-last line, got %q", separatorLine)
	}
}

func TestView_EmptyFooter_NoExtraBlankLine(t *testing.T) {
	m := newTestAppModel()
	m.activePage = PageCreation // ActionBindings() returns nil -> empty footer

	content := renderSizedView(m, 80, 24)

	// With no footer, the view should not exceed the window height
	if viewHeight := lipgloss.Height(content); viewHeight > 24 {
		t.Fatalf("expected view height <= 24, got %d", viewHeight)
	}
}
