package tui

import (
	"devora/internal/tui/components"
	"devora/internal/workspace"
	"fmt"
	"os"
	"path/filepath"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
)

// Add-repo specific messages
type addRepoSubmitMsg struct {
	repoName       string
	worktreeDirName string
}

type addRepoDoneMsg struct{}

type addRepoErrorMsg struct {
	err error
}

type addRepoField int

const (
	fieldAddRepoList addRepoField = iota
	fieldAddRepoPostfix
	fieldAddRepoSubmit
)

type AddRepoModel struct {
	workspacePath string
	repoNames     []string
	repoList      components.ListModel
	postfixInput  components.TextInputModel
	focused       addRepoField
	navMode       bool

	// Creation progress
	showProgress bool
	status       string
	repoStatus   string
	errMsg       string

	styles  Styles
	width   int
	height  int
}

func NewAddRepoModel(
	palette ThemePalette,
	workspacePath string,
	repoNames []string,
) AddRepoModel {
	styles := NewStyles(palette)

	list := components.NewListModel(styles.Diamond)
	var items []components.ListItem
	for _, name := range repoNames {
		items = append(items, components.ListItem{Content: name, Value: name})
	}
	list.SetItems(items)
	list.Height = 8

	postfix := components.NewTextInputModel(
		lipgloss.NewStyle(),
		lipgloss.NewStyle().Foreground(palette.Panel),
	)
	postfix.Placeholder = "e.g. ref, alt..."

	return AddRepoModel{
		workspacePath: workspacePath,
		repoNames:     repoNames,
		repoList:      list,
		postfixInput:  postfix,
		focused:       fieldAddRepoList,
		styles:        styles,
	}
}

func (m AddRepoModel) Init() tea.Cmd {
	return nil
}

func (m AddRepoModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case addRepoDoneMsg:
		return m, tea.Quit

	case addRepoErrorMsg:
		m.errMsg = msg.err.Error()
		return m, nil

	case tea.KeyPressMsg:
		key := msg.String()

		// Global: ctrl+c always quits
		if key == "ctrl+c" {
			return m, tea.Quit
		}

		// Error state (showProgress with errMsg): esc/q quit
		if m.showProgress && m.errMsg != "" {
			if key == "q" || key == "esc" {
				return m, tea.Quit
			}
			return m, nil
		}

		// In-progress (no error): all keys are no-op
		if m.showProgress {
			return m, nil
		}

		// Form state: quit bindings with two-stage esc
		if key == "esc" || key == "q" {
			if m.focused == fieldAddRepoPostfix && !m.navMode {
				if key == "esc" {
					// First esc: unfocus text input, enter nav mode
					m.postfixInput.Blur()
					m.navMode = true
					return m, nil
				}
				// q in insert mode: pass through to text input
			} else {
				// Non-text field, or nav mode: quit
				return m, tea.Quit
			}
		}

		// Focus cycling
		if key == "tab" {
			switch m.focused {
			case fieldAddRepoList:
				m.repoList.Reset()
				m.postfixInput.Focus()
				m.focused = fieldAddRepoPostfix
				m.navMode = false
			case fieldAddRepoPostfix:
				m.postfixInput.Blur()
				m.focused = fieldAddRepoSubmit
			case fieldAddRepoSubmit:
				m.focused = fieldAddRepoList
			}
			return m, nil
		}
		if key == "shift+tab" {
			switch m.focused {
			case fieldAddRepoList:
				m.focused = fieldAddRepoSubmit
			case fieldAddRepoPostfix:
				m.postfixInput.Blur()
				m.focused = fieldAddRepoList
			case fieldAddRepoSubmit:
				m.postfixInput.Focus()
				m.focused = fieldAddRepoPostfix
				m.navMode = false
			}
			return m, nil
		}

		// Enter on list selects repo and moves to postfix
		if key == "enter" && m.focused == fieldAddRepoList {
			m.repoList.Reset()
			m.postfixInput.Focus()
			m.focused = fieldAddRepoPostfix
			m.navMode = false
			return m, nil
		}

		// Submit
		if key == "enter" && (m.focused == fieldAddRepoSubmit || m.focused == fieldAddRepoPostfix) {
			return m, m.handleSubmit()
		}

		// Delegate to focused component
		m.errMsg = ""
		switch m.focused {
		case fieldAddRepoList:
			m.repoList.HandleKey(key)
		case fieldAddRepoPostfix:
			m.postfixInput.HandleKey(key)
		}
		return m, nil
	}
	return m, nil
}

func (m *AddRepoModel) handleSubmit() tea.Cmd {
	selected := m.repoList.SelectedItem()
	if selected == nil {
		m.errMsg = "No repo selected"
		return nil
	}
	repoName := selected.Value.(string)

	postfix := m.postfixInput.Value
	worktreeDirName := repoName
	if postfix != "" {
		worktreeDirName = repoName + "-" + postfix
	}

	targetPath := filepath.Join(m.workspacePath, worktreeDirName)
	if _, err := filepath.Abs(targetPath); err == nil {
		if exists(targetPath) {
			m.errMsg = fmt.Sprintf("Directory '%s' already exists. Use a different postfix.", worktreeDirName)
			return nil
		}
	}

	m.showProgress = true
	m.status = "Adding worktree..."
	m.repoStatus = worktreeDirName + ": preparing..."
	m.errMsg = ""

	return func() tea.Msg {
		err := workspace.MakeAndPrepareWorkTree(m.workspacePath, repoName, worktreeDirName)
		if err != nil {
			return addRepoErrorMsg{err: err}
		}
		if err := workspace.EnsureWorkspaceCLAUDEMD(m.workspacePath); err != nil {
			return addRepoErrorMsg{err: fmt.Errorf("worktree created, but failed to write CLAUDE.md: %w", err)}
		}
		return addRepoDoneMsg{}
	}
}

func (m AddRepoModel) View() tea.View {
	var content string

	if m.showProgress {
		var body string
		body = "  " + m.styles.Title.Render("Adding Repo") + "\n\n"
		body += "  " + m.status + "\n"
		if m.repoStatus != "" {
			body += "    " + m.repoStatus + "\n"
		}
		if m.errMsg != "" {
			body += "\n" + m.styles.NotifyError.Render("\u2717 "+m.errMsg) + "\n"
		}
		content = body
	} else {
		var body string
		body = "  " + m.styles.Title.Render("Add Repo") + "\n\n"
		body += "  " + m.styles.Muted.Render("Repo") + "\n"
		body += m.repoList.View() + "\n\n"
		body += "  " + m.styles.Muted.Render("Name Postfix (optional)") + "\n"
		body += "  " + m.postfixInput.View() + "\n\n"

		submitLabel := "[ Add Repo ]"
		if m.focused == fieldAddRepoSubmit {
			body += "  " + m.styles.SubmitFocused.Render(submitLabel)
		} else {
			body += "  " + m.styles.SubmitBlurred.Render(submitLabel)
		}
		if m.errMsg != "" {
			body += "\n\n" + m.styles.NotifyError.Render("\u2717 "+m.errMsg)
		}
		content = body
	}

	var bindings []components.KeyBinding
	if m.showProgress && m.errMsg != "" {
		bindings = []components.KeyBinding{{Key: "esc/q", Desc: "Quit"}}
	} else if m.showProgress {
		// In-progress: no key hints
	} else if m.focused == fieldAddRepoPostfix && !m.navMode {
		bindings = []components.KeyBinding{
			{Key: "tab", Desc: "Next Field"},
			{Key: "enter", Desc: "Submit"},
			{Key: "esc", Desc: "Unfocus"},
		}
	} else {
		bindings = []components.KeyBinding{
			{Key: "tab", Desc: "Next Field"},
			{Key: "enter", Desc: "Submit"},
			{Key: "esc/q", Desc: "Quit"},
		}
	}

	footer := components.RenderFooter(
		bindings,
		m.styles.FooterKey, m.styles.FooterDesc, m.styles.Separator,
		m.width,
	)

	view := lipgloss.JoinVertical(lipgloss.Left, content, footer)
	v := tea.NewView(view)
	v.AltScreen = true
	return v
}

func exists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// RunAddRepo runs the add-repo TUI.
func RunAddRepo(
	themePath string,
	workspacePath string,
	repoNames []string,
) error {
	palette := LoadTheme(themePath)
	model := NewAddRepoModel(palette, workspacePath, repoNames)
	p := tea.NewProgram(model)
	_, err := p.Run()
	return err
}
