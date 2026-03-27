package tui

// Page transition messages
type showWorkspaceListMsg struct{}
type refreshWorkspacesMsg struct{}
type showNewTaskMsg struct{}
type showSettingsMsg struct{}
type showRegisterRepoMsg struct {
	fromSettings bool
}
type showProfileRegistrationMsg struct {
	fromSettings bool
}

// Workspace data types

type WorkspaceCategory int

const (
	CategoryActiveWithSession WorkspaceCategory = iota
	CategoryActiveNoSession
	CategoryInactive
	CategoryInvalid
)

type RepoGitStatus struct {
	Branch  string
	IsClean bool
}

type WorkspaceInfo struct {
	Path            string
	Name            string
	Category        WorkspaceCategory
	TaskTitle       string
	Repos           []string
	RepoGitStatuses map[string]RepoGitStatus
	InvalidReason   string
}

type NewTaskResult struct {
	RepoNames []string
	TaskName  string
}

type WorkspaceReadyResult struct {
	WorkspacePath string
	SessionName   string
}

// Data loading messages
type workspacesLoadedMsg struct {
	workspaces []WorkspaceInfo
}

type workspaceLoadErrorMsg struct {
	err error
}

// Workspace creation messages
type creationStatusMsg struct {
	text string
}

type repoAddedMsg struct {
	name string
}

type repoReadyMsg struct {
	name string
}

type creationDoneMsg struct {
	workspacePath string
	sessionName   string
}

type creationErrorMsg struct {
	err error
}

// Delete messages
type requestDeleteMsg struct {
	info WorkspaceInfo
}

type deleteCompleteMsg struct{}

type deleteErrorMsg struct {
	err error
}

// Deactivation messages
type deactivateCompleteMsg struct{}

type deactivateErrorMsg struct {
	err error
}

// Notification messages
type notifyMsg struct {
	text    string
	isError bool
}

type notifyClearMsg struct {
	seq int
}

// Spinner messages
type spinnerTickMsg struct{}

// Profile messages
type profileActivatedMsg struct{}
