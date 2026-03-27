package tui

import (
	"fmt"
	"sort"
	"strings"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"devora/internal/tui/components"
	"devora/internal/workspace"
)

type FilterMode int

const (
	FilterActive FilterMode = iota + 1
	FilterInactive
	FilterAll
)

type WorkspaceListModel struct {
	allWorkspaces []WorkspaceInfo
	list          components.ListModel
	filterMode    FilterMode
	loading       bool
	styles        *Styles
	width, height int
}

func NewWorkspaceListModel(styles *Styles) WorkspaceListModel {
	return WorkspaceListModel{
		list:       components.NewListModel(lipgloss.NewStyle()),
		filterMode: FilterActive,
		loading:    true,
		styles:     styles,
	}
}

func (m *WorkspaceListModel) SetWorkspaces(ws []WorkspaceInfo) {
	m.allWorkspaces = ws
	m.loading = false
	m.rebuildListItems()
}

func (m *WorkspaceListModel) SetFilter(mode FilterMode) {
	m.filterMode = mode
	m.rebuildListItems()
}

func (m *WorkspaceListModel) SetSize(w, h int) {
	m.width = w
	m.height = h
	// Account for border (2 lines top/bottom) and title line
	m.list.Height = h - 2
}

func (m *WorkspaceListModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		key := msg.String()

		switch key {
		case "1":
			m.SetFilter(FilterActive)
			return nil
		case "2":
			m.SetFilter(FilterInactive)
			return nil
		case "3":
			m.SetFilter(FilterAll)
			return nil
		case "enter":
			info := m.SelectedWorkspace()
			if info == nil {
				return nil
			}
			selected := *info
			return func() tea.Msg { return selected }
		case "d":
			return m.handleDeactivateRequest()
		case "D":
			return m.handleDeleteRequest()
		case "r":
			return func() tea.Msg { return refreshWorkspacesMsg{} }
		case "n":
			return func() tea.Msg { return showNewTaskMsg{} }
		case "R":
			return func() tea.Msg { return showRegisterRepoMsg{} }
		case "p":
			return cycleProfileCmd()
		case "s":
			return func() tea.Msg { return showSettingsMsg{} }
		case "q", "esc":
			return tea.Quit
		default:
			m.list.HandleKey(key)
		}
	}
	return nil
}

func (m *WorkspaceListModel) View() string {
	if m.loading {
		return "  " + m.styles.Muted.Render("Loading workspaces...")
	}

	filtered := m.filteredWorkspaces()
	if len(filtered) == 0 {
		return "\n  " + m.styles.Title.Render(m.emptyMessage()) + "\n"
	}

	cardWidth := m.width - 6 // margin left(2) + border(2) + padding(4) accounted in style
	if cardWidth > 90 {
		cardWidth = 90
	}
	if cardWidth < 20 {
		cardWidth = 20
	}
	innerWidth := cardWidth - 6 // border(2) + padding(4)
	if innerWidth < 10 {
		innerWidth = 10
	}

	var cards []string
	for i, ws := range filtered {
		isSelected := i == m.list.Cursor
		cards = append(cards, m.renderCard(ws, isSelected, innerWidth, cardWidth))
	}

	return strings.Join(cards, "\n")
}

func (m *WorkspaceListModel) SelectedWorkspace() *WorkspaceInfo {
	item := m.list.SelectedItem()
	if item == nil {
		return nil
	}
	info, ok := item.Value.(WorkspaceInfo)
	if !ok {
		return nil
	}
	return &info
}

func (m *WorkspaceListModel) borderTitle() string {
	switch m.filterMode {
	case FilterActive:
		return "Active Workspaces"
	case FilterInactive:
		return "Inactive Workspaces"
	case FilterAll:
		return "All Workspaces"
	}
	return "Workspaces"
}

func (m *WorkspaceListModel) ActiveFilterMode() FilterMode {
	return m.filterMode
}

func (m WorkspaceListModel) ActionBindings() []components.KeyBinding {
	return []components.KeyBinding{
		{Key: "n", Desc: "New Task"},
		{Key: "d", Desc: "Deactivate"},
		{Key: "D", Desc: "Delete"},
		{Key: "r", Desc: "Refresh"},
		{Key: "R", Desc: "Register Repo"},
		{Key: "p", Desc: "Cycle Profile"},
		{Key: "s", Desc: "Settings"},
		{Key: "q", Desc: "Quit"},
	}
}

// rebuildListItems recomputes the filtered list from allWorkspaces.
func (m *WorkspaceListModel) rebuildListItems() {
	filtered := m.filteredWorkspaces()
	items := make([]components.ListItem, len(filtered))
	for i, ws := range filtered {
		items[i] = components.ListItem{
			Content: "", // rendering is done in View() via renderCard
			Value:   ws,
		}
	}
	m.list.SetItems(items)
}

func (m *WorkspaceListModel) filteredWorkspaces() []WorkspaceInfo {
	switch m.filterMode {
	case FilterActive:
		var result []WorkspaceInfo
		for _, ws := range m.allWorkspaces {
			if ws.Category == CategoryActiveWithSession || ws.Category == CategoryActiveNoSession {
				result = append(result, ws)
			}
		}
		return result
	case FilterInactive:
		var result []WorkspaceInfo
		for _, ws := range m.allWorkspaces {
			if ws.Category == CategoryInactive {
				result = append(result, ws)
			}
		}
		return result
	case FilterAll:
		sorted := make([]WorkspaceInfo, len(m.allWorkspaces))
		copy(sorted, m.allWorkspaces)
		order := map[WorkspaceCategory]int{
			CategoryActiveWithSession: 0,
			CategoryActiveNoSession:   1,
			CategoryInactive:          2,
			CategoryInvalid:           3,
		}
		sort.SliceStable(sorted, func(i, j int) bool {
			return order[sorted[i].Category] < order[sorted[j].Category]
		})
		return sorted
	}
	return nil
}

func (m *WorkspaceListModel) renderCard(ws WorkspaceInfo, isSelected bool, innerWidth, cardWidth int) string {
	var lines []string

	// Title line with status badge
	title := ws.TaskTitle
	if title == "" {
		title = ws.Name
	}

	titlePrefix := ""
	if isSelected {
		titlePrefix = m.styles.Diamond.Render("\u25c6") + " "
	}

	var badge string
	switch ws.Category {
	case CategoryActiveWithSession, CategoryActiveNoSession:
		badge = m.styles.BadgeActive.Render("ACTIVE")
	case CategoryInactive:
		badge = m.styles.BadgeIdle.Render("IDLE")
	case CategoryInvalid:
		badge = m.styles.BadgeInvalid.Render("INVALID")
	}

	titleText := titlePrefix + m.styles.CardTitle.Render(title)
	titleWidth := lipgloss.Width(titleText)
	badgeWidth := lipgloss.Width(badge)
	gap := innerWidth - titleWidth - badgeWidth
	if gap < 1 {
		gap = 1
	}
	lines = append(lines, titleText+strings.Repeat(" ", gap)+badge)

	// Repo table
	repoNames := make([]string, 0, len(ws.RepoGitStatuses))
	for name := range ws.RepoGitStatuses {
		repoNames = append(repoNames, name)
	}
	sort.Strings(repoNames)

	if len(repoNames) > 0 {
		lines = append(lines, "")
		for _, name := range repoNames {
			status := ws.RepoGitStatuses[name]
			repoCol := m.styles.RepoName.Width(16).Render(name)
			branchCol := m.styles.RepoBranch.Width(14).Render(status.Branch)
			var cleanCol string
			if status.IsClean {
				cleanCol = m.styles.RepoClean.Render("\u2713 clean")
			} else {
				cleanCol = m.styles.RepoDirty.Render("\u2717 dirty")
			}
			lines = append(lines, "  "+repoCol+branchCol+cleanCol)
		}
	}

	// Invalid reason
	if ws.Category == CategoryInvalid && ws.InvalidReason != "" {
		lines = append(lines, "")
		lines = append(lines, m.styles.Muted.Render(ws.InvalidReason))
	}

	content := strings.Join(lines, "\n")

	// Wrap in card border
	var card lipgloss.Style
	if isSelected {
		card = m.styles.CardSelected.Width(innerWidth)
	} else {
		card = m.styles.CardUnselected.Width(innerWidth)
	}

	return card.Render(content)
}

func (m *WorkspaceListModel) emptyMessage() string {
	switch m.filterMode {
	case FilterActive:
		return "No active workspaces"
	case FilterInactive:
		return "No inactive workspaces"
	case FilterAll:
		return "No workspaces"
	}
	return ""
}

func (m *WorkspaceListModel) handleDeactivateRequest() tea.Cmd {
	info := m.SelectedWorkspace()
	if info == nil {
		return nil
	}

	if blockerCmd := checkWorkspaceBlockers(info, "deactivate"); blockerCmd != nil {
		return blockerCmd
	}

	wsPath := info.Path
	return func() tea.Msg {
		err := workspace.DeactivateWorkspace(wsPath)
		if err != nil {
			return deactivateErrorMsg{err: err}
		}
		return deactivateCompleteMsg{}
	}
}

func (m *WorkspaceListModel) handleDeleteRequest() tea.Cmd {
	info := m.SelectedWorkspace()
	if info == nil {
		return nil
	}

	if blockerCmd := checkWorkspaceBlockers(info, "delete"); blockerCmd != nil {
		return blockerCmd
	}

	selected := *info
	return func() tea.Msg { return requestDeleteMsg{info: selected} }
}

// checkWorkspaceBlockers checks whether a workspace has an active session,
// non-detached branches, or uncommitted changes. Returns a notification command
// for the first blocker found, or nil if the workspace is clear.
func checkWorkspaceBlockers(info *WorkspaceInfo, action string) tea.Cmd {
	if info.Category == CategoryActiveWithSession {
		msg := notifyMsg{
			text:    fmt.Sprintf("Cannot %s: workspace has an active terminal session", action),
			isError: true,
		}
		return func() tea.Msg { return msg }
	}

	for _, repoName := range sortedRepoStatusNames(info.RepoGitStatuses) {
		status := info.RepoGitStatuses[repoName]
		if status.Branch != "HEAD" {
			msg := notifyMsg{
				text:    fmt.Sprintf("Cannot %s: repo '%s' is not on HEAD", action, repoName),
				isError: true,
			}
			return func() tea.Msg { return msg }
		}
		if !status.IsClean {
			msg := notifyMsg{
				text:    fmt.Sprintf("Cannot %s: repo '%s' has uncommitted changes", action, repoName),
				isError: true,
			}
			return func() tea.Msg { return msg }
		}
	}

	return nil
}

func sortedRepoStatusNames(statuses map[string]RepoGitStatus) []string {
	names := make([]string, 0, len(statuses))
	for name := range statuses {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}
