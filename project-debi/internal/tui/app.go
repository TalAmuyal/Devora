package tui

import (
	"devora/internal/config"
	"devora/internal/process"
	"devora/internal/style"
	"devora/internal/task"
	"devora/internal/terminal"
	"devora/internal/tui/components"
	"devora/internal/version"
	"devora/internal/workspace"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
)

type Page int

const (
	PageWorkspaceList Page = iota
	PageNewTask
	PageCreation
	PageDeleteConfirm
	PageRegisterRepo
	PageSettings
	PageProfileRegistration
	PageStartupError
)

// AppResult is returned when the app exits with a result.
type AppResult struct {
	SelectedWorkspace *WorkspaceInfo
	NewWorkspace      *WorkspaceReadyResult
}

type AppModel struct {
	activePage Page
	result     *AppResult

	workspaceList WorkspaceListModel
	newTask       NewTaskModel
	creation      CreationModel
	deleteConfirm DeleteConfirmModel
	registerRepo  RegisterRepoModel
	settings      SettingsModel
	profileReg    ProfileRegModel
	startupError  StartupErrorModel

	styles      Styles
	palette     style.ThemePalette
	repoNames   []string
	profileName string
	version     string

	creationCh chan tea.Msg

	notifyText    string
	notifyIsError bool
	notifySeq     int

	width  int
	height int
}

func NewAppModel(
	palette style.ThemePalette,
	repoNames []string,
	showProfileRegistration bool,
	startupError string,
) AppModel {
	styles := NewStyles(palette)

	profileName := ""
	if profile := config.GetActiveProfile(); profile != nil {
		profileName = profile.Name
	}

	m := AppModel{
		activePage:  PageWorkspaceList,
		styles:      styles,
		palette:     palette,
		repoNames:   repoNames,
		profileName: profileName,
		version:     version.Get(),

		workspaceList: NewWorkspaceListModel(&styles),
		newTask:       NewNewTaskModel(repoNames, &styles),
		creation:      NewCreationModel(&styles),
		deleteConfirm: NewDeleteConfirmModel(&styles),
		registerRepo:  NewRegisterRepoModel(&styles),
		settings:      NewSettingsModel(&styles),
		profileReg:    NewProfileRegModel(&styles, !showProfileRegistration),
		startupError:  NewStartupErrorModel(&styles, startupError),
	}

	if startupError != "" {
		m.activePage = PageStartupError
	} else if showProfileRegistration {
		m.activePage = PageProfileRegistration
	}

	return m
}

func (m AppModel) Init() tea.Cmd {
	if m.activePage == PageStartupError || m.activePage == PageProfileRegistration {
		return nil
	}
	return m.loadWorkspacesCmd()
}

func (m AppModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		// Layout: header (2) + tab bar (1, workspace list only) + content + notify (0-1) + footer (2)
		chromeHeight := 4 // header(2) + footer separator(1) + footer(1)
		contentHeight := m.height - chromeHeight
		// Workspace list gets 1 less for the tab bar
		wsContentHeight := contentHeight - 1
		m.workspaceList.SetSize(m.width, wsContentHeight)
		m.newTask.SetSize(m.width, contentHeight)
		m.creation.SetSize(m.width, contentHeight)
		m.deleteConfirm.SetSize(m.width, contentHeight)
		m.registerRepo.SetSize(m.width, contentHeight)
		m.settings.SetSize(m.width, contentHeight)
		m.profileReg.SetSize(m.width, contentHeight)
		m.startupError.SetSize(m.width, contentHeight)
		return m, nil

	// Page transitions
	case showWorkspaceListMsg:
		repoNames, err := config.GetRegisteredRepoNames()
		if err == nil {
			m.repoNames = repoNames
		}
		m.activePage = PageWorkspaceList
		return m, nil
	case refreshWorkspacesMsg:
		m.workspaceList.loading = true
		return m, m.loadWorkspacesCmd()
	case showNewTaskMsg:
		m.newTask.Reset()
		m.newTask.SetRepoNames(m.repoNames)
		m.activePage = PageNewTask
		return m, nil
	case showSettingsMsg:
		profile := config.GetActiveProfile()
		profileName := ""
		if profile != nil {
			profileName = profile.Name
		}
		m.settings.Activate(profileName)
		m.activePage = PageSettings
		return m, nil
	case showRegisterRepoMsg:
		m.registerRepo.Reset()
		m.registerRepo.returnToSettings = msg.fromSettings
		m.activePage = PageRegisterRepo
		return m, nil
	case showProfileRegistrationMsg:
		m.profileReg.Reset()
		m.profileReg.hasBack = true
		m.profileReg.returnToSettings = msg.fromSettings
		m.activePage = PageProfileRegistration
		return m, nil

	case openChangelogMsg:
		return m, m.openChangelogCmd()

	// Data messages
	case workspaceLoadErrorMsg:
		m.workspaceList.loading = false
		return m, func() tea.Msg {
			return notifyMsg{text: fmt.Sprintf("Failed to load workspaces: %v", msg.err), isError: true}
		}
	case workspacesLoadedMsg:
		m.workspaceList.SetWorkspaces(msg.workspaces)
		return m, nil

	// New task submission
	case NewTaskResult:
		m.creation.Reset()
		m.activePage = PageCreation
		ch := make(chan tea.Msg, 20)
		m.creationCh = ch
		return m, tea.Batch(m.createWorkspaceCmd(msg, ch), listenCreation(ch), spinnerTick())

	// Spinner
	case spinnerTickMsg:
		if m.activePage == PageCreation && m.creation.errMsg == "" {
			m.creation.spinnerFrame = (m.creation.spinnerFrame + 1) % len(spinnerFrames)
			return m, spinnerTick()
		}
		return m, nil

	// Creation progress
	case creationStatusMsg:
		m.creation.status = msg.text
		return m, listenCreation(m.creationCh)
	case repoAddedMsg:
		m.creation.repoOrder = append(m.creation.repoOrder, msg.name)
		m.creation.repoStates[msg.name] = "preparing..."
		return m, listenCreation(m.creationCh)
	case repoReadyMsg:
		m.creation.repoStates[msg.name] = "ready"
		return m, listenCreation(m.creationCh)
	case creationDoneMsg:
		m.result = &AppResult{
			NewWorkspace: &WorkspaceReadyResult{
				WorkspacePath: msg.workspacePath,
				SessionName:   msg.sessionName,
			},
		}
		return m, tea.Quit
	case creationErrorMsg:
		m.creation.errMsg = msg.err.Error()
		return m, nil

	// Deactivate
	case deactivateCompleteMsg:
		return m, tea.Batch(
			m.loadWorkspacesCmd(),
			func() tea.Msg {
				return notifyMsg{text: "Workspace deactivated", isError: false}
			},
		)
	case deactivateErrorMsg:
		return m, func() tea.Msg {
			return notifyMsg{text: fmt.Sprintf("Failed to deactivate: %v", msg.err), isError: true}
		}

	// Delete
	case requestDeleteMsg:
		m.deleteConfirm.SetWorkspace(&msg.info)
		m.activePage = PageDeleteConfirm
		return m, nil
	case deleteCompleteMsg:
		m.activePage = PageWorkspaceList
		return m, m.loadWorkspacesCmd()
	case deleteErrorMsg:
		m.deleteConfirm.errMsg = msg.err.Error()
		return m, nil

	// Workspace selected from list
	case WorkspaceInfo:
		m.result = &AppResult{SelectedWorkspace: &msg}
		return m, tea.Quit

	// Profile activated
	case profileActivatedMsg:
		repoNames, err := config.GetRegisteredRepoNames()
		if err != nil {
			return m, func() tea.Msg {
				return notifyMsg{text: fmt.Sprintf("Failed to load repos: %v", err), isError: true}
			}
		}
		m.repoNames = repoNames
		m.activePage = PageWorkspaceList
		m.workspaceList.loading = true
		profile := config.GetActiveProfile()
		if profile != nil {
			m.profileName = profile.Name
		}
		return m, m.loadWorkspacesCmd()

	// Notification
	case notifyMsg:
		m.notifySeq++
		m.notifyText = msg.text
		m.notifyIsError = msg.isError
		seq := m.notifySeq
		return m, func() tea.Msg {
			time.Sleep(4 * time.Second)
			return notifyClearMsg{seq: seq}
		}
	case notifyClearMsg:
		if msg.seq == m.notifySeq {
			m.notifyText = ""
			m.notifyIsError = false
		}
		return m, nil

	// Key handling - delegate to active page
	case tea.KeyPressMsg:
		// Global binding: ctrl+c always quits the entire app
		if msg.String() == "ctrl+c" {
			return m, tea.Quit
		}

		var cmd tea.Cmd
		switch m.activePage {
		case PageWorkspaceList:
			cmd = m.workspaceList.Update(msg)
		case PageNewTask:
			cmd = m.newTask.Update(msg)
		case PageCreation:
			cmd = m.creation.Update(msg)
		case PageDeleteConfirm:
			cmd = m.deleteConfirm.Update(msg)
		case PageRegisterRepo:
			cmd = m.registerRepo.Update(msg)
		case PageSettings:
			cmd = m.settings.Update(msg)
		case PageProfileRegistration:
			cmd = m.profileReg.Update(msg)
		case PageStartupError:
			cmd = m.startupError.Update(msg)
		}
		return m, cmd
	}

	return m, nil
}

func (m AppModel) View() tea.View {
	var content string
	var pageTitle string
	var actionBindings []components.KeyBinding

	switch m.activePage {
	case PageWorkspaceList:
		content = m.workspaceList.View()
		pageTitle = m.workspaceList.borderTitle()
		actionBindings = m.workspaceList.ActionBindings()
	case PageNewTask:
		content = m.newTask.View()
		pageTitle = m.newTask.borderTitle()
		actionBindings = m.newTask.ActionBindings()
	case PageCreation:
		content = m.creation.View()
		pageTitle = m.creation.borderTitle()
		actionBindings = m.creation.ActionBindings()
	case PageDeleteConfirm:
		content = m.deleteConfirm.View()
		pageTitle = m.deleteConfirm.borderTitle()
		actionBindings = m.deleteConfirm.ActionBindings()
	case PageRegisterRepo:
		content = m.registerRepo.View()
		pageTitle = m.registerRepo.borderTitle()
		actionBindings = m.registerRepo.ActionBindings()
	case PageSettings:
		content = m.settings.View()
		pageTitle = m.settings.borderTitle()
		actionBindings = m.settings.ActionBindings()
	case PageProfileRegistration:
		content = m.profileReg.View()
		pageTitle = m.profileReg.borderTitle()
		actionBindings = m.profileReg.ActionBindings()
	case PageStartupError:
		content = m.startupError.View()
		pageTitle = m.startupError.borderTitle()
		actionBindings = m.startupError.ActionBindings()
	}

	// Header: DEVORA · PageTitle                    profile
	header := m.renderHeader(pageTitle)

	// Tab bar (workspace list only)
	tabBar := ""
	if m.activePage == PageWorkspaceList {
		tabBar = m.renderTabBar()
	}

	// Notification
	notify := ""
	if m.notifyText != "" {
		if m.notifyIsError {
			notify = m.styles.NotifyError.Render("\u2717 " + m.notifyText)
		} else {
			notify = m.styles.NotifySuccess.Render("\u2713 " + m.notifyText)
		}
	}

	// Footer
	footer := components.RenderFooter(actionBindings, m.styles.FooterKey, m.styles.FooterDesc, m.styles.Separator, m.width, m.version)

	// Assemble the top section and pad it so the footer sticks to the bottom.
	var topParts []string
	topParts = append(topParts, header)
	if tabBar != "" {
		topParts = append(topParts, tabBar)
	}
	topParts = append(topParts, content)
	if notify != "" {
		topParts = append(topParts, notify)
	}
	topSection := lipgloss.JoinVertical(lipgloss.Left, topParts...)

	if footer != "" {
		topHeight := lipgloss.Height(topSection)
		footerHeight := lipgloss.Height(footer)
		padLines := m.height - topHeight - footerHeight
		if padLines > 0 {
			topSection += strings.Repeat("\n", padLines)
		}
	}

	var view string
	if footer != "" {
		view = lipgloss.JoinVertical(lipgloss.Left, topSection, footer)
	} else {
		view = topSection
	}
	v := tea.NewView(view)
	v.AltScreen = true
	return v
}

func (m AppModel) renderHeader(pageTitle string) string {
	left := "  " + m.styles.Brand.Render("DEVORA") +
		m.styles.HeaderDot.Render("  \u00b7  ") +
		m.styles.HeaderPageTitle.Render(pageTitle)

	right := ""
	if m.profileName != "" {
		right = m.styles.HeaderProfile.Render(m.profileName) + "  "
	}

	leftWidth := lipgloss.Width(left)
	rightWidth := lipgloss.Width(right)
	gap := m.width - leftWidth - rightWidth
	if gap < 0 {
		gap = 0
	}

	headerLine := left + strings.Repeat(" ", gap) + right
	sep := m.renderSeparator()
	return headerLine + "\n" + sep
}

func (m AppModel) renderTabBar() string {
	filterMode := m.workspaceList.ActiveFilterMode()
	tabs := []struct {
		label string
		mode  FilterMode
	}{
		{"Active", FilterActive},
		{"Inactive", FilterInactive},
		{"All", FilterAll},
	}

	var parts []string
	for _, tab := range tabs {
		if tab.mode == filterMode {
			parts = append(parts, m.styles.TabActive.Render("\u25b8 "+tab.label))
		} else {
			parts = append(parts, m.styles.TabInactive.Render(tab.label))
		}
	}
	return "  " + strings.Join(parts, "    ")
}

func (m AppModel) renderSeparator() string {
	w := m.width - 4
	if w < 0 {
		w = 0
	}
	line := strings.Repeat("\u2500", w)
	return "  " + m.styles.Separator.Render(line)
}

// Result returns the app result after exit.
func (m AppModel) Result() *AppResult {
	return m.result
}

// openChangelogCmd returns a command that opens the changelog in a new Kitty tab.
func (m AppModel) openChangelogCmd() tea.Cmd {
	return func() tea.Msg {
		resourcesDir := os.Getenv("DEVORA_RESOURCES_DIR")
		if resourcesDir == "" {
			return notifyMsg{text: "Changelog not available outside the app bundle", isError: true}
		}
		configDir := os.Getenv("KITTY_CONFIG_DIRECTORY")
		if configDir == "" {
			return notifyMsg{text: "Kitty config directory not set", isError: true}
		}
		changelogPath := filepath.Join(resourcesDir, "CHANGELOG.md")
		glowStylePath := filepath.Join(configDir, "glow-theme.json")
		shell := os.Getenv("SHELL")
		if shell == "" {
			shell = "/bin/sh"
		}
		_, err := process.GetOutput([]string{
			"kitty", "@", "launch",
			"--type=tab",
			"--tab-title", "Changelog",
			shell, "-l", "-i",
			"-c", fmt.Sprintf("glow --style %s --pager %s", glowStylePath, changelogPath),
		})
		if err != nil {
			return notifyMsg{text: fmt.Sprintf("Failed to open changelog: %v", err), isError: true}
		}
		return nil
	}
}

// loadWorkspacesCmd returns a command that gathers workspace info in the background.
func (m AppModel) loadWorkspacesCmd() tea.Cmd {
	return func() tea.Msg {
		workspaces, err := gatherWorkspaceInfos()
		if err != nil {
			return workspaceLoadErrorMsg{err: err}
		}
		return workspacesLoadedMsg{workspaces: workspaces}
	}
}

// createWorkspaceCmd returns a command that creates a workspace.
// Progress is reported through the provided channel. The goroutine sends
// creationStatusMsg, repoAddedMsg, and repoReadyMsg as work progresses,
// and ends with either creationDoneMsg or creationErrorMsg.
func (m AppModel) createWorkspaceCmd(result NewTaskResult, progressCh chan<- tea.Msg) tea.Cmd {
	return func() tea.Msg {
		defer close(progressCh)

		progressCh <- creationStatusMsg{text: "Resolving workspace..."}

		workspacesRoot, err := config.GetWorkspacesRootPath()
		if err != nil {
			return creationErrorMsg{err: err}
		}

		wsPath, _ := workspace.SearchAvailableWorkspace(result.RepoNames, workspacesRoot)

		if wsPath == "" {
			progressCh <- creationStatusMsg{text: "Creating workspace directory..."}

			wsPath, err = workspace.CreateWorkspaceDirectory(workspacesRoot)
			if err != nil {
				return creationErrorMsg{err: err}
			}

			unlock, err := workspace.LockWorkspace(wsPath)
			if err != nil {
				return creationErrorMsg{err: err}
			}
			defer unlock.Close()

			if len(result.RepoNames) > 1 {
				if err := workspace.WriteWorkspaceCLAUDEMD(wsPath); err != nil {
					return creationErrorMsg{err: fmt.Errorf("write workspace CLAUDE.md: %w", err)}
				}
			}

			progressCh <- creationStatusMsg{text: "Preparing worktrees..."}

			for _, repoName := range result.RepoNames {
				progressCh <- repoAddedMsg{name: repoName}
			}

			type worktreeResult struct {
				name string
				err  error
			}
			resCh := make(chan worktreeResult, len(result.RepoNames))

			for _, repoName := range result.RepoNames {
				go func(name string) {
					err := workspace.MakeAndPrepareWorkTree(wsPath, name, name)
					resCh <- worktreeResult{name: name, err: err}
				}(repoName)
			}

			for range result.RepoNames {
				res := <-resCh
				if res.err != nil {
					return creationErrorMsg{err: res.err}
				}
				progressCh <- repoReadyMsg{name: res.name}
			}

			if err := workspace.MarkInitialized(wsPath); err != nil {
				return creationErrorMsg{err: fmt.Errorf("mark workspace initialized: %w", err)}
			}
		}

		progressCh <- creationStatusMsg{text: "Creating task..."}

		taskPath := workspace.GetWorkspaceTaskPath(wsPath)
		if err := task.Create(result.TaskName, taskPath); err != nil {
			return creationErrorMsg{err: err}
		}

		return creationDoneMsg{
			workspacePath: wsPath,
			sessionName:   result.TaskName,
		}
	}
}

// gatherWorkspaceInfos collects workspace information from the filesystem and terminal sessions.
func gatherWorkspaceInfos() ([]WorkspaceInfo, error) {
	workspacesRoot, err := config.GetWorkspacesRootPath()
	if err != nil {
		return nil, fmt.Errorf("get workspaces root: %w", err)
	}

	// Start session listing in parallel
	var sessions []terminal.Session
	var sessionsErr error
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		backend := terminal.NewBackend()
		sessions, sessionsErr = backend.ListSessions()
	}()

	// Scan workspaces
	wsPaths, err := workspace.GetWorkspaces(workspacesRoot)
	if err != nil {
		return nil, fmt.Errorf("list workspaces: %w", err)
	}

	type pendingWS struct {
		path  string
		repos []string
	}
	var pending []pendingWS
	for _, wsPath := range wsPaths {
		if workspace.IsWorkspaceLocked(wsPath) {
			continue
		}
		repos, err := workspace.GetWorkspaceRepos(wsPath)
		if err != nil {
			repos = nil
		}
		pending = append(pending, pendingWS{path: wsPath, repos: repos})
	}

	// Start git queries in parallel
	type gitResult struct {
		wsIdx    int
		repoName string
		branch   string
		isClean  bool
		err      error
	}
	gitCh := make(chan gitResult, len(pending)*5)
	var gitWg sync.WaitGroup
	for i, ws := range pending {
		for _, repoName := range ws.repos {
			gitWg.Add(1)
			go func(idx int, wsPath, repo string) {
				defer gitWg.Done()
				repoPath := filepath.Join(wsPath, repo)
				branch, branchErr := workspace.GetRepoBranch(repoPath)
				clean, cleanErr := workspace.IsRepoClean(repoPath)
				err := branchErr
				if err == nil {
					err = cleanErr
				}
				gitCh <- gitResult{wsIdx: idx, repoName: repo, branch: branch, isClean: clean, err: err}
			}(i, ws.path, repoName)
		}
	}
	go func() {
		gitWg.Wait()
		close(gitCh)
	}()

	// Collect git results
	gitStatuses := make([]map[string]RepoGitStatus, len(pending))
	for i := range pending {
		gitStatuses[i] = make(map[string]RepoGitStatus)
	}
	for res := range gitCh {
		if res.err == nil {
			gitStatuses[res.wsIdx][res.repoName] = RepoGitStatus{
				Branch:  res.branch,
				IsClean: res.isClean,
			}
		}
	}

	// Wait for sessions
	wg.Wait()

	sessionPaths := make(map[string]bool)
	if sessionsErr == nil {
		for _, s := range sessions {
			sessionPaths[s.RootPath] = true
		}
	}

	// Build workspace infos
	var infos []WorkspaceInfo
	for i, ws := range pending {
		info := WorkspaceInfo{
			Path:            ws.path,
			Name:            filepath.Base(ws.path),
			Repos:           ws.repos,
			RepoGitStatuses: gitStatuses[i],
		}

		if !workspace.IsInitialized(ws.path) {
			info.Category = CategoryInvalid
			info.InvalidReason = "Not initialized"
		} else if workspace.HasTask(ws.path) {
			// Read task title
			taskPath := workspace.GetWorkspaceTaskPath(ws.path)
			data, err := os.ReadFile(taskPath)
			if err == nil {
				var taskData struct {
					Title string `json:"title"`
				}
				if json.Unmarshal(data, &taskData) == nil {
					info.TaskTitle = taskData.Title
				}
			}

			workDir, err := workspace.GetSessionWorkingDirectory(ws.path)
			if err != nil {
				workDir = ws.path
			}
			if sessionPaths[workDir] {
				info.Category = CategoryActiveWithSession
			} else {
				info.Category = CategoryActiveNoSession
			}
		} else {
			info.Category = CategoryInactive
		}

		infos = append(infos, info)
	}

	return infos, nil
}

// RunWorkspaceUI runs the workspace management TUI and returns the result.
func RunWorkspaceUI(
	themePath string,
	repoNames []string,
	showProfileRegistration bool,
	startupError string,
) (*AppResult, error) {
	palette := style.LoadKittyTheme(themePath)
	model := NewAppModel(palette, repoNames, showProfileRegistration, startupError)
	p := tea.NewProgram(model)
	finalModel, err := p.Run()
	if err != nil {
		return nil, err
	}
	return finalModel.(AppModel).Result(), nil
}

// CreateAndAttachSession creates a terminal session for a workspace and attaches to it.
func CreateAndAttachSession(wsPath, sessionName string) error {
	backend := terminal.NewBackend()

	app := config.GetDefaultTerminalApp("shell")
	timeout := config.TerminalSessionCreationTimeoutSeconds(3)

	workDir, err := workspace.GetSessionWorkingDirectory(wsPath)
	if err != nil {
		return err
	}

	_, err = terminal.CreateAndAttach(backend, sessionName, workDir, app, timeout)
	return err
}

// cycleProfileCmd returns a command that cycles to the next profile.
func cycleProfileCmd() tea.Cmd {
	return func() tea.Msg {
		profiles := config.GetProfiles()
		if len(profiles) < 2 {
			return nil
		}
		active := config.GetActiveProfile()
		if active == nil {
			config.SetActiveProfile(&profiles[0])
			return profileActivatedMsg{}
		}
		currentIdx := -1
		for i, p := range profiles {
			if p.RootPath == active.RootPath {
				currentIdx = i
				break
			}
		}
		nextIdx := (currentIdx + 1) % len(profiles)
		config.SetActiveProfile(&profiles[nextIdx])
		return profileActivatedMsg{}
	}
}

// listenCreation returns a command that reads the next message from the creation channel.
func listenCreation(ch <-chan tea.Msg) tea.Cmd {
	return func() tea.Msg {
		msg, ok := <-ch
		if !ok {
			return nil
		}
		return msg
	}
}

