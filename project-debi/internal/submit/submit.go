// Package submit implements debi's `submit` command: commit local changes,
// create a tracker task (if configured), create a feature branch, push it,
// open a GitHub PR, and optionally enable auto-merge.
//
// Run is the entry point. All external dependencies are injected via
// package-level function vars so tests can stub them without touching the
// filesystem, network, or subprocess layer.
package submit

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"runtime"

	"charm.land/lipgloss/v2"
	"golang.org/x/sync/errgroup"

	"devora/internal/config"
	"devora/internal/gh"
	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/tasktracker"
)

// ErrNotDetached is returned when Run is called from a non-detached HEAD.
// The CLI layer detects it via errors.Is and translates to *cli.UsageError.
var ErrNotDetached = errors.New("submit requires detached HEAD")

// Options controls submit behavior.
type Options struct {
	Message     string // required: commit message, task title, PR title.
	Description string // optional: appended to PR body.
	Draft       bool   // create PR as draft.
	Blocked     bool   // skip enabling auto-merge.
	OpenBrowser bool   // open the PR URL in the default browser on success.
	JSONOutput  bool   // emit a single JSON object instead of human output.
	SkipTracker bool   // treat as no tracker configured, even if one is.
	Verbose     bool   // show live git/gh subprocess output.
	Quiet       bool   // print only the final PR URL on stdout.
}

// JSONOutput is the shape emitted when Options.JSONOutput is true.
type JSONOutput struct {
	Number     int    `json:"number"`
	URL        string `json:"url"`
	Title      string `json:"title"`
	Branch     string `json:"branch"`
	BaseBranch string `json:"base_branch"`
	Draft      bool   `json:"draft"`
	TaskURL    string `json:"task_url,omitempty"`
}

// --- Stubbable dependencies ---

var (
	getCurrentBranchOrDetached = defaultCurrentBranchOrDetached
	addAllAndCommit            = git.AddAllAndCommit
	generateBranchName         = git.GenerateBranchName
	createAndCheckoutBranch    = git.CreateAndCheckoutBranch
	setBranchConfig            = git.SetBranchConfig
	pushSetUpstream            = git.PushSetUpstream

	getGHRepo         = gh.GetRepo
	createGHPR        = gh.CreatePR
	enableGHAutoMerge = gh.EnableAutoMergeSquash

	newTracker = tasktracker.NewForActiveProfile

	getBranchPrefix = config.GetBranchPrefix
	openBrowser     = defaultOpenBrowser
	getGHPRView     = defaultGetGHPRView
)

// stderr is the sink for warnings that must not corrupt w. In JSON mode w is
// the real stdout and must contain only the JSON payload, so warnings go here.
// Stubbable in tests.
var stderr io.Writer = os.Stderr

// defaultCurrentBranchOrDetached wraps git.CurrentBranchOrDetached into the
// zero-arg signature Run uses. Tests replace getCurrentBranchOrDetached with a
// zero-arg stub, so this adapter keeps the stubbable-var signature uniform.
func defaultCurrentBranchOrDetached() (string, error) {
	return git.CurrentBranchOrDetached()
}

// defaultOpenBrowser opens url in the OS default browser via `open` on macOS
// or `xdg-open` on Linux. Uses passthrough so the user sees any child output.
func defaultOpenBrowser(url string) error {
	var command []string
	switch runtime.GOOS {
	case "darwin":
		command = []string{"open", url}
	default:
		command = []string{"xdg-open", url}
	}
	return process.RunPassthrough(command)
}

// defaultGetGHPRView fetches PR number/title for JSON output. Called only in
// --json mode. Used so tests can inject a stub without going through gh.
func defaultGetGHPRView(url string) (gh.PRSummary, error) {
	pr, err := gh.GetPRForBranch(url)
	if err != nil {
		return gh.PRSummary{}, err
	}
	if pr == nil {
		return gh.PRSummary{}, errors.New("gh pr view returned no PR for the given URL")
	}
	return *pr, nil
}

// --- Styles (Catppuccin Mocha — match internal/prstatus) ---

var (
	greenStyle  = lipgloss.NewStyle().Foreground(lipgloss.Color("#A6E3A1"))
	yellowStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#F9E2AF"))
	cyanStyle   = lipgloss.NewStyle().Foreground(lipgloss.Color("#89DCEB"))
)

// --- Run ---

// Run executes the submit flow: validate preconditions, commit changes,
// (optionally) create a tracker task, create+push a feature branch, open a
// PR, and (unless Blocked) enable auto-merge.
//
// Output modes:
//   - JSON (opts.JSONOutput): a single JSON object on w; progress to io.Discard.
//   - Quiet (opts.Quiet): only the PR URL on w (no prefix, no styling); errors to stderr.
//   - Verbose (opts.Verbose): live git/gh subprocess output + progress lines + full human summary.
//   - Normal (default): subprocesses silenced, concise progress log with checkmarks, URL on its own line.
func Run(w io.Writer, opts Options) error {
	if opts.Message == "" {
		return errors.New("--message is required")
	}

	// Subprocess output: silenced in every mode except Verbose so the
	// default/quiet output stays concise. Threaded into every git/gh call.
	var execOpts []process.ExecOption
	if !opts.Verbose {
		execOpts = []process.ExecOption{process.WithSilent()}
	}

	// Progress sink: where checkmark/warning lines go. Suppressed in JSON
	// and Quiet modes so only the final payload reaches w.
	progress := w
	if opts.JSONOutput || opts.Quiet {
		progress = io.Discard
	}

	// Detached-HEAD guard: submit must start from detached HEAD. A branch
	// means the user has not yet merged the previous PR.
	branch, err := getCurrentBranchOrDetached()
	if err != nil {
		return fmt.Errorf("check current branch: %w", err)
	}
	if branch != "" {
		return fmt.Errorf("%w (currently on branch %q). The current PR needs to be reviewed and merged before further work is performed.", ErrNotDetached, branch)
	}

	// Stage and commit.
	if opts.Verbose {
		fmt.Fprintln(progress, cyanStyle.Render("\u2192 Staging and committing..."))
	}
	if err := addAllAndCommit(opts.Message, execOpts...); err != nil {
		return fmt.Errorf("commit changes: %w", err)
	}
	fmt.Fprintln(progress, greenStyle.Render("\u2713 Committed"))

	// Resolve tracker. SkipTracker forces nil.
	var tracker tasktracker.Tracker
	if !opts.SkipTracker {
		tracker, err = newTracker()
		if err != nil {
			return fmt.Errorf("resolve task-tracker: %w", err)
		}
	}

	// Pre-fetch: run only the calls we actually need, and only run them in
	// parallel if both are needed. Unused pre-fetches add latency and a
	// pointless errgroup.
	var whoami string
	var repoInfo gh.RepoInfo
	needWhoAmI := tracker != nil
	needRepo := opts.JSONOutput
	switch {
	case needWhoAmI && needRepo:
		g, _ := errgroup.WithContext(context.Background())
		g.Go(func() error {
			var gerr error
			whoami, gerr = tracker.WhoAmI()
			return gerr
		})
		g.Go(func() error {
			var gerr error
			repoInfo, gerr = getGHRepo()
			return gerr
		})
		if err := g.Wait(); err != nil {
			return fmt.Errorf("pre-fetch: %w", err)
		}
	case needWhoAmI:
		whoami, err = tracker.WhoAmI()
		if err != nil {
			return fmt.Errorf("tracker whoami: %w", err)
		}
	case needRepo:
		repoInfo, err = getGHRepo()
		if err != nil {
			return fmt.Errorf("gh repo view: %w", err)
		}
	}

	// Create tracker task if a tracker is configured.
	var task tasktracker.Task
	if tracker != nil {
		task, err = tracker.CreateTask(tasktracker.CreateTaskRequest{
			Title:       opts.Message,
			AssigneeGID: whoami,
		})
		if err != nil {
			return fmt.Errorf("create tracker task: %w", err)
		}
		fmt.Fprintln(progress, greenStyle.Render("\u2713 Task created: "+task.URL))
	}

	// Generate branch name, create it, store the task ID (if any), push.
	prefix := getBranchPrefix("feature")
	branchName := generateBranchName(prefix, opts.Message)

	if err := createAndCheckoutBranch(branchName, execOpts...); err != nil {
		return fmt.Errorf("create branch %s: %w", branchName, err)
	}
	if tracker != nil && task.ID != "" {
		// Key is "task-id" — provider-agnostic. The provider-specific
		// identifier lives only in the value.
		if err := setBranchConfig(branchName, "task-id", task.ID, execOpts...); err != nil {
			return fmt.Errorf("set branch task-id: %w", err)
		}
	}
	if err := pushSetUpstream(branchName, execOpts...); err != nil {
		return fmt.Errorf("push branch: %w", err)
	}
	fmt.Fprintln(progress, greenStyle.Render("\u2713 Pushed "+branchName))

	// Build PR body: tracker prefix first, description after (with a blank
	// line separator when both are present).
	body := ""
	if tracker != nil && task.ID != "" {
		body = tracker.PRBodyPrefix(task.ID)
	}
	if opts.Description != "" {
		if body != "" {
			body = body + "\n\n" + opts.Description
		} else {
			body = opts.Description
		}
	}

	// Create the PR. Distinguish "already exists" because the error is
	// user-actionable and we want to surface the existing URL.
	prURL, err := createGHPR(opts.Message, body, opts.Draft)
	if err != nil {
		var exists *gh.PRAlreadyExistsError
		if errors.As(err, &exists) {
			// Route the warning to stderr so stdout (especially in JSON or
			// Quiet mode) stays clean for piping to scripts.
			if exists.ExistingURL != "" {
				fmt.Fprintln(stderr, yellowStyle.Render("\u26a0 PR already exists: "+exists.ExistingURL))
			} else {
				fmt.Fprintln(stderr, yellowStyle.Render("\u26a0 A pull request for this branch already exists"))
			}
			return fmt.Errorf("create PR: %w", err)
		}
		return fmt.Errorf("create PR: %w", err)
	}

	// Post-PR ops. Auto-merge is best-effort; a failure warns but does not
	// fail the command. Warnings land on stderr in JSON/Quiet modes to keep
	// stdout clean; in Normal/Verbose they go to progress (which == w).
	if !opts.Blocked {
		if err := enableGHAutoMerge(prURL, execOpts...); err != nil {
			sink := progress
			if opts.JSONOutput || opts.Quiet {
				sink = stderr
			}
			fmt.Fprintln(sink, yellowStyle.Render(fmt.Sprintf("\u26a0 Could not enable auto-merge: %v", err)))
		} else {
			fmt.Fprintln(progress, greenStyle.Render("\u2713 Auto-merge enabled"))
		}
	}

	// Emit output.
	switch {
	case opts.JSONOutput:
		return emitJSON(w, opts, prURL, branchName, repoInfo.DefaultBranch, task)
	case opts.Quiet:
		// Plain URL, no prefix, no styling — easy to pipe to jq/xargs/etc.
		fmt.Fprintln(w, prURL)
	case opts.Verbose:
		// Existing verbose summary: checkmark prefix, branch + task lines.
		fmt.Fprintln(w, greenStyle.Render("\u2713 PR created: "+prURL))
		fmt.Fprintln(w, "  Branch: "+branchName)
		if task.URL != "" {
			fmt.Fprintln(w, "  Task:   "+task.URL)
		}
	default:
		// Normal mode: concise checkmark followed by the URL on its own
		// line so it's easy to copy or script against.
		fmt.Fprintln(w, greenStyle.Render("\u2713 PR created"))
		fmt.Fprintln(w, prURL)
	}

	// Optionally open the browser. Failures here are informational only.
	if opts.OpenBrowser {
		if err := openBrowser(prURL); err != nil {
			sink := progress
			if opts.JSONOutput || opts.Quiet {
				sink = stderr
			}
			fmt.Fprintln(sink, yellowStyle.Render(fmt.Sprintf("\u26a0 Could not open browser: %v", err)))
		}
	}

	return nil
}

// emitJSON writes the JSONOutput struct to w. It fetches the PR number and
// title via getGHPRView; if that fails, it emits a minimal object using the
// values we already have (URL, branch, opts.Message as title, draft, task URL)
// and logs a warning — the command's primary goal (PR created) has already
// succeeded, so a cosmetic fetch failure must not turn it into a hard error.
func emitJSON(w io.Writer, opts Options, prURL, branchName, baseBranch string, task tasktracker.Task) error {
	out := JSONOutput{
		URL:        prURL,
		Branch:     branchName,
		BaseBranch: baseBranch,
		Draft:      opts.Draft,
		Title:      opts.Message,
		TaskURL:    task.URL,
	}

	// A cosmetic lookup failure after the PR is successfully created must
	// not fail the command. The warning is silently swallowed in JSON mode
	// to keep the payload clean; human mode never uses this code path.
	if summary, err := getGHPRView(prURL); err == nil {
		out.Number = summary.Number
		if summary.Title != "" {
			out.Title = summary.Title
		}
	}

	data, jerr := json.MarshalIndent(out, "", "  ")
	if jerr != nil {
		return fmt.Errorf("marshal JSON output: %w", jerr)
	}
	fmt.Fprintln(w, string(data))
	return nil
}
