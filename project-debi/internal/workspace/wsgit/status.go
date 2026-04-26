package wsgit

import (
	"context"
	"errors"
	"fmt"
	"io"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"charm.land/lipgloss/v2"

	"devora/internal/gh"
	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/workspace"
)

// Catppuccin Mocha palette. Values mirror internal/close/close.go:101-106;
// duplicated here so wsgit doesn't import the close command package.
var (
	greenStyle  = lipgloss.NewStyle().Foreground(lipgloss.Color("#A6E3A1"))
	redStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("#F38BA8"))
	yellowStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#F9E2AF"))
	cyanStyle   = lipgloss.NewStyle().Foreground(lipgloss.Color("#89DCEB"))
	mutedStyle  = lipgloss.NewStyle().Foreground(lipgloss.Color("#6C7086"))

	mergedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#A6E3A1")).Bold(true)
)

// fetchTimeout caps each per-repo `git fetch origin` so a hung remote doesn't
// stall the workspace-wide summary.
const fetchTimeout = 30 * time.Second

// RunStatus implements workspace-mode `debi gst`: fan-out per-repo queries in
// parallel, then render an aligned summary table. Always returns nil — gst is
// a read-only summary and per-repo errors are surfaced inline.
func RunStatus(w io.Writer, wsPath string) error {
	repos, err := workspace.GetWorkspaceRepos(wsPath)
	if err != nil {
		return fmt.Errorf("list workspace repos: %w", err)
	}

	// ghMissing is set the first time any goroutine hits ErrGHNotInstalled.
	// All goroutines then skip gh entirely and the renderer prints a single
	// footnote rather than per-row noise.
	var ghMissing atomic.Bool

	results := make([]RepoStatus, len(repos))
	var wg sync.WaitGroup
	for i, repo := range repos {
		wg.Add(1)
		go func(idx int, name string) {
			defer wg.Done()
			results[idx] = gatherRepoStatus(filepath.Join(wsPath, name), name, &ghMissing)
		}(i, repo)
	}
	wg.Wait()

	sort.Slice(results, func(i, j int) bool {
		return results[i].Name < results[j].Name
	})

	renderStatus(w, wsPath, results, ghMissing.Load())
	return nil
}

// gatherRepoStatus collects the per-repo data shown in the gst table: a best-
// effort fetch, branch, working-tree counts, behind-count, and PR status. PR
// lookup is skipped when HEAD is detached or when gh has been reported missing.
func gatherRepoStatus(repoPath, name string, ghMissing *atomic.Bool) RepoStatus {
	status := RepoStatus{Name: name, BehindOrigin: -1}

	if err := bestEffortFetch(repoPath); err != nil {
		status.FetchFailed = true
	}

	branch, err := git.CurrentBranchOrDetached(process.WithCwd(repoPath))
	if err != nil {
		status.Err = err
		return status
	}
	status.Branch = branch

	porcelain, err := process.GetOutputRaw(
		[]string{"git", "status", "--porcelain=v1", "-z"},
		process.WithCwd(repoPath),
	)
	if err != nil {
		status.Err = err
	} else {
		status.Counts = parsePorcelain(porcelain)
	}

	mainBranch, err := git.DefaultBranchNameWithFallback(process.WithCwd(repoPath))
	if err == nil {
		status.BehindOrigin = countBehindOrigin(repoPath, mainBranch)
	}

	if branch == "" {
		// Detached: skip gh entirely. `gh pr view ""` is broken behavior.
		status.PRState = PRStateNone
		return status
	}

	if ghMissing.Load() {
		return status
	}

	pr, err := getPRForBranch(branch, process.WithCwd(repoPath))
	if err != nil {
		if errors.Is(err, gh.ErrGHNotInstalled) {
			ghMissing.Store(true)
			return status
		}
		status.PRError = err
		return status
	}
	if pr == nil {
		status.PRState = PRStateNone
		return status
	}
	status.PRState = classifyPR(pr)
	return status
}

// bestEffortFetch runs `git fetch origin` with prompts disabled and a timeout
// so a hung remote can't stall a parallel fan-out. Returns the raw error so
// callers can choose to surface it (gcl) or treat it as a stale-data marker
// (gst).
func bestEffortFetch(repoPath string) error {
	ctx, cancel := context.WithTimeout(context.Background(), fetchTimeout)
	defer cancel()
	_, err := process.GetOutput(
		[]string{"git", "fetch", "origin"},
		process.WithCwd(repoPath),
		process.WithContext(ctx),
		process.WithExtraEnv(
			"GIT_TERMINAL_PROMPT=0",
			"GIT_SSH_COMMAND=ssh -o BatchMode=yes",
		),
	)
	return err
}

// countBehindOrigin returns the number of commits in origin/<mainBranch> that
// are missing from HEAD. Returns -1 when rev-list fails (e.g., the ref doesn't
// exist locally) — the renderer prints "?" in that case.
func countBehindOrigin(repoPath, mainBranch string) int {
	out, err := process.GetOutput(
		[]string{"git", "rev-list", "--count", "HEAD..origin/" + mainBranch},
		process.WithCwd(repoPath),
	)
	if err != nil {
		return -1
	}
	n, err := strconv.Atoi(out)
	if err != nil {
		return -1
	}
	return n
}

// classifyPR maps gh.PRSummary into the typed PRState the renderer expects.
// Unrecognized states return PRStateUnknown so misleading defaults don't leak
// into the table.
func classifyPR(pr *gh.PRSummary) PRState {
	switch pr.State {
	case "OPEN":
		return PRStateOpen
	case "CLOSED":
		return PRStateClosed
	case "MERGED":
		return PRStateMerged
	}
	return PRStateUnknown
}

// renderStatus prints the aligned status table, optional warnings, and the
// SUMMARY block.
func renderStatus(w io.Writer, wsPath string, results []RepoStatus, ghMissing bool) {
	wsName := filepath.Base(wsPath)
	fmt.Fprintf(w, "WORKSPACE  %s (%d repos)\n\n", wsName, len(results))

	nameWidth := len("REPO")
	branchWidth := len("BRANCH")
	for _, r := range results {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
		display := branchDisplay(r.Branch)
		if len(display) > branchWidth {
			branchWidth = len(display)
		}
	}

	headerStyle := mutedStyle
	header := fmt.Sprintf(
		"  %-*s  %-*s  %5s  %5s  %5s  %6s  %s",
		nameWidth, "REPO",
		branchWidth, "BRANCH",
		"STAGE", "UNSTG", "UNTRK", "BEHIND", "PR",
	)
	fmt.Fprintln(w, headerStyle.Render(header))

	for _, r := range results {
		fmt.Fprintln(w, formatStatusRow(r, nameWidth, branchWidth, ghMissing))
	}

	hasFetchFailures := false
	for _, r := range results {
		if r.FetchFailed {
			hasFetchFailures = true
			fmt.Fprintln(w)
			fmt.Fprintf(w, "%s  %s: fetch failed (using cached data; counts may be stale)\n",
				yellowStyle.Render("WARN"), r.Name)
		}
	}
	if !hasFetchFailures {
		// keep a single blank line between table and summary regardless
		fmt.Fprintln(w)
	}

	if ghMissing {
		fmt.Fprintln(w, mutedStyle.Render("gh not installed; PR status unavailable"))
	}

	renderSummary(w, results)
}

func formatStatusRow(r RepoStatus, nameWidth, branchWidth int, ghMissing bool) string {
	branchText := branchDisplay(r.Branch)

	stageText, stageStyle := classifyCount(r.Counts.Staged, greenStyle)
	unstgText, unstgStyle := classifyCount(r.Counts.Unstaged, yellowStyle)
	untrkText, untrkStyle := classifyCount(r.Counts.Untracked, redStyle)
	behindText, behindStyle := classifyBehind(r.BehindOrigin, r.FetchFailed)
	prText, prStyle := classifyPRDisplay(r, ghMissing)

	branchStyle := cyanStyle
	if r.Branch == "" {
		branchStyle = mutedStyle
	}

	return fmt.Sprintf(
		"  %s  %s  %s  %s  %s  %s  %s",
		padCell(r.Name, nameWidth, lipgloss.NewStyle()),
		padCell(branchText, branchWidth, branchStyle),
		padCell(stageText, 5, stageStyle),
		padCell(unstgText, 5, unstgStyle),
		padCell(untrkText, 5, untrkStyle),
		padCell(behindText, 6, behindStyle),
		prStyle.Render(prText),
	)
}

// padCell renders text with style, then right-pads with plain spaces so the
// visible width equals width. Computing padding from the unstyled text avoids
// the column-drift you get from formatting an already-ANSI-escaped string
// with `%-*s`.
func padCell(text string, width int, style lipgloss.Style) string {
	rendered := style.Render(text)
	pad := width - len(text)
	if pad <= 0 {
		return rendered
	}
	return rendered + strings.Repeat(" ", pad)
}

func branchDisplay(branch string) string {
	if branch == "" {
		return "HEAD"
	}
	return branch
}

func classifyCount(n int, color lipgloss.Style) (string, lipgloss.Style) {
	if n == 0 {
		return ".", mutedStyle
	}
	return strconv.Itoa(n), color
}

func classifyBehind(n int, fetchFailed bool) (string, lipgloss.Style) {
	if n < 0 {
		return "?", mutedStyle
	}
	text := strconv.Itoa(n)
	if fetchFailed {
		text += "*"
	}
	if n == 0 {
		return text, greenStyle
	}
	return text, yellowStyle
}

func classifyPRDisplay(r RepoStatus, ghMissing bool) (string, lipgloss.Style) {
	if ghMissing {
		return "—", mutedStyle
	}
	if r.PRError != nil {
		return "?", mutedStyle
	}
	switch r.PRState {
	case PRStateOpen:
		return "open", cyanStyle
	case PRStateClosed:
		return "closed", redStyle
	case PRStateMerged:
		return "merged", mergedStyle
	default:
		return "No", mutedStyle
	}
}

func renderSummary(w io.Writer, results []RepoStatus) {
	dirty := 0
	detached := 0
	fetchFailed := 0
	withBranch := 0
	for _, r := range results {
		if r.Counts.Staged > 0 || r.Counts.Unstaged > 0 || r.Counts.Untracked > 0 {
			dirty++
		}
		if r.Branch == "" {
			detached++
		} else {
			withBranch++
		}
		if r.FetchFailed {
			fetchFailed++
		}
	}

	prOpen := 0
	prClosed := 0
	prMerged := 0
	prNone := 0
	for _, r := range results {
		switch r.PRState {
		case PRStateOpen:
			prOpen++
		case PRStateClosed:
			prClosed++
		case PRStateMerged:
			prMerged++
		default:
			prNone++
		}
	}

	fmt.Fprintln(w)
	fmt.Fprintf(w, "SUMMARY  %d dirty, %d detached, %d fetch-failed, %d of %d with branch info\n",
		dirty, detached, fetchFailed, withBranch, len(results))
	fmt.Fprintf(w, "         PRs: %d open, %d closed, %d merged, %d none\n",
		prOpen, prClosed, prMerged, prNone)
}
