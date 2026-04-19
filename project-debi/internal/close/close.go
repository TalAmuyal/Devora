// Package closecmd implements the `debi close` command: marks the tracker
// task as complete, deletes the remote branch, and returns the working tree
// to detached HEAD on origin/<default>, deleting the local branch.
//
// The package is named `closecmd` (not `close`) so callers' source doesn't
// visually shadow the Go builtin `close`. Import as:
//
//	closecmd "devora/internal/close"
package closecmd

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"
	"sync"

	"charm.land/lipgloss/v2"
	"golang.org/x/sync/errgroup"

	"devora/internal/gh"
	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/tasktracker"
)

// --- Public API ---

// Options configures Run.
type Options struct {
	// TaskURL, when set, overrides the task ID stored at
	// branch.<current-branch>.task-id. Requires a configured task-tracker.
	TaskURL string
	// SkipTracker bypasses the tracker entirely (no task completion).
	SkipTracker bool
	// Force skips the "PR is still open" confirmation prompt.
	Force bool
	// Verbose shows live git/gh subprocess output + all progress lines.
	Verbose bool
	// Quiet suppresses progress lines; only the final "✓ Closed" reaches w.
	Quiet bool
}

// Sentinels detected by the CLI layer via errors.Is to produce the right
// user-facing wrapper / exit code.
var (
	ErrDetached        = errors.New("close cannot run from detached HEAD")
	ErrProtectedBranch = errors.New("close cannot run on protected branch")
	ErrAborted         = errors.New("aborted")
	ErrNoTrackerForURL = errors.New("--task-url requires a configured task-tracker")
)

// --- Stubbable package-level vars (tests override in stubCloseDeps) ---

var (
	getCurrentBranchOrDetached = defaultGetCurrentBranchOrDetached
	isProtectedBranch          = git.IsProtectedBranch
	defaultBranchName          = defaultDefaultBranchName
	getBranchConfig            = defaultGetBranchConfig
	hasRemoteBranch            = defaultHasRemoteBranch
	hasLocalBranch             = defaultHasLocalBranch
	deleteRemoteBranch         = git.DeleteRemoteBranch
	deleteLocalBranch          = git.DeleteLocalBranch
	fetchOrigin                = git.FetchOrigin
	checkoutDetach             = git.CheckoutDetach

	getGHPRForBranch = gh.GetPRForBranch

	newTracker = tasktracker.NewForActiveProfile

	// readLine reads one line from r. Stubbable in tests. The default
	// implementation wraps r with a bufio.Reader and reads up to the first
	// newline; Run passes os.Stdin.
	readLine = defaultReadLine
)

// Thin trampolines around git package functions that take variadic process
// options we don't use at this layer. Separate from the stubbable vars so
// signatures stay trivial to override in tests.

func defaultGetCurrentBranchOrDetached() (string, error) { return git.CurrentBranchOrDetached() }
func defaultDefaultBranchName() (string, error)          { return git.DefaultBranchNameWithFallback() }
func defaultGetBranchConfig(branch, key string) (string, error) {
	return git.GetBranchConfig(branch, key)
}
func defaultHasRemoteBranch(branch string) bool { return git.HasRemoteBranch(branch) }
func defaultHasLocalBranch(branch string) bool  { return git.HasLocalBranch(branch) }

func defaultReadLine(r io.Reader) (string, error) {
	line, err := bufio.NewReader(r).ReadString('\n')
	if err != nil {
		return "", err
	}
	return line, nil
}

// --- Colors (Catppuccin Mocha), matched to internal/prstatus ---

var (
	greenStyle  = lipgloss.NewStyle().Foreground(lipgloss.Color("#A6E3A1"))
	redStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("#F38BA8"))
	yellowStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#F9E2AF"))
	cyanStyle   = lipgloss.NewStyle().Foreground(lipgloss.Color("#89DCEB"))
)

// --- confirm helper ---

// confirm prints message to w and reads one line from stdin via readLine.
// Returns true iff the trimmed-lowercased response is "y" or "yes". Any
// other response (including Enter / empty) returns false. Stdin read errors
// are wrapped and propagated.
func confirm(w io.Writer, message string) (bool, error) {
	fmt.Fprint(w, message)
	line, err := readLine(os.Stdin)
	if err != nil {
		return false, fmt.Errorf("read response: %w", err)
	}
	r := strings.TrimSpace(strings.ToLower(line))
	return r == "y" || r == "yes", nil
}

// --- Run ---

// Run executes the close flow.
//
// Output modes:
//   - Verbose: live git subprocess output + all progress lines.
//   - Quiet: only "✓ Closed" reaches w; subprocesses silenced.
//   - Normal (default): concise checkmark progress log + "✓ Closed"; subprocesses silenced.
//
// The open-PR confirmation prompt always prints regardless of mode — user
// consent is sacred.
func Run(w io.Writer, opts Options) error {
	// Subprocess output: silenced except in Verbose mode.
	var execOpts []process.ExecOption
	if !opts.Verbose {
		execOpts = []process.ExecOption{process.WithSilent()}
	}

	// Progress sink: where checkmark/warning lines go. Discard in Quiet.
	progress := w
	if opts.Quiet {
		progress = io.Discard
	}

	branch, err := getCurrentBranchOrDetached()
	if err != nil {
		return err
	}
	if branch == "" {
		return ErrDetached
	}
	if isProtectedBranch(branch) {
		return fmt.Errorf("%w: %q", ErrProtectedBranch, branch)
	}

	tracker, err := resolveTracker(opts)
	if err != nil {
		return err
	}

	taskID, err := resolveTaskID(progress, opts, tracker, branch)
	if err != nil {
		return err
	}

	if !opts.Force {
		// PR-open prompt uses w directly (not progress) so the user sees it
		// even in Quiet mode; consent cannot be suppressed.
		if aborted, err := checkOpenPR(w, branch); err != nil {
			return err
		} else if aborted {
			return ErrAborted
		}
	}

	defBranch, err := defaultBranchName()
	if err != nil {
		return err
	}

	runParallelPhase(progress, tracker, taskID, branch, execOpts)

	if err := returnToDetachedHead(progress, branch, defBranch, execOpts); err != nil {
		return err
	}

	// Final marker is always printed, regardless of mode — this is the
	// script-parseable success signal.
	fmt.Fprintf(w, "%s Closed\n", greenStyle.Render("\u2713"))
	return nil
}

// resolveTracker returns nil when SkipTracker is set; otherwise it builds the
// tracker from config. A nil tracker with a nil error means "no tracker is
// configured" — callers check for that.
func resolveTracker(opts Options) (tasktracker.Tracker, error) {
	if opts.SkipTracker {
		return nil, nil
	}
	return newTracker()
}

// resolveTaskID picks the task ID from --task-url or the branch's git config,
// prints a warning when a tracker is configured but no ID is found, and
// returns ErrNoTrackerForURL when --task-url is used without a tracker.
func resolveTaskID(w io.Writer, opts Options, tracker tasktracker.Tracker, branch string) (string, error) {
	if opts.TaskURL != "" {
		if tracker == nil {
			return "", ErrNoTrackerForURL
		}
		return tracker.ParseTaskURL(opts.TaskURL), nil
	}

	id, _ := getBranchConfig(branch, "task-id")
	if tracker != nil && id == "" {
		fmt.Fprintf(w, "%s No task ID associated with branch %s; skipping task completion.\n",
			yellowStyle.Render("\u26a0"), branch)
	}
	return id, nil
}

// checkOpenPR queries GitHub for the branch's PR. When a PR is OPEN, it
// prompts the user for confirmation. Returns (aborted, err) where aborted
// means the user declined. PR-check errors are non-fatal (warning printed,
// aborted=false).
func checkOpenPR(w io.Writer, branch string) (bool, error) {
	pr, err := getGHPRForBranch(branch)
	if err != nil {
		fmt.Fprintf(w, "%s Could not check PR status: %v\n",
			yellowStyle.Render("\u26a0"), err)
		return false, nil
	}
	if pr == nil || pr.State != "OPEN" {
		return false, nil
	}

	fmt.Fprintf(w, "%s PR #%d is still open: %s\n",
		redStyle.Render("!"), pr.Number, pr.URL)

	ok, err := confirm(w, "Are you sure you want to close this work item? [y/N]: ")
	if err != nil {
		return false, err
	}
	if !ok {
		fmt.Fprintf(w, "%s Aborted\n", cyanStyle.Render("\u2192"))
		return true, nil
	}
	return false, nil
}

// syncedWriter serializes concurrent writes to an underlying io.Writer so the
// goroutines in runParallelPhase can print warnings/successes without racing
// on a non-thread-safe sink (e.g., bytes.Buffer in tests, or any caller that
// doesn't guarantee atomic writes).
type syncedWriter struct {
	mu sync.Mutex
	w  io.Writer
}

func (s *syncedWriter) Write(p []byte) (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.w.Write(p)
}

// runParallelPhase runs the best-effort operations (task completion + remote
// branch deletion) concurrently. Each goroutine prints its own warning on
// failure and returns nil to the group, so the group's return error is always
// nil and is intentionally ignored — warnings have already been surfaced.
// Writes to w are serialized through syncedWriter to avoid data races.
func runParallelPhase(w io.Writer, tracker tasktracker.Tracker, taskID, branch string, execOpts []process.ExecOption) {
	sw := &syncedWriter{w: w}
	var g errgroup.Group

	if tracker != nil && taskID != "" {
		g.Go(func() error {
			if err := tracker.CompleteTask(taskID); err != nil {
				fmt.Fprintf(sw, "%s Could not mark task as complete: %v\n",
					yellowStyle.Render("\u26a0"), err)
				return nil
			}
			fmt.Fprintf(sw, "%s Marked task as complete\n",
				greenStyle.Render("\u2713"))
			return nil
		})
	}

	if hasRemoteBranch(branch) {
		g.Go(func() error {
			if err := deleteRemoteBranch(branch, execOpts...); err != nil {
				fmt.Fprintf(sw, "%s Could not delete remote branch: %v\n",
					yellowStyle.Render("\u26a0"), err)
				return nil
			}
			fmt.Fprintf(sw, "%s Deleted remote branch\n",
				greenStyle.Render("\u2713"))
			return nil
		})
	} else {
		fmt.Fprintf(sw, "%s Remote branch already deleted\n",
			cyanStyle.Render("\u2192"))
	}

	// Goroutines never surface errors; ignoring g.Wait() is deliberate.
	_ = g.Wait()
}

// returnToDetachedHead fetches origin, checks out detached on
// origin/<default>, and force-deletes the local branch. When branch is
// already the default, it prints a skip warning and returns nil. Fetch and
// checkout failures are fatal; local-branch deletion failure is non-fatal.
func returnToDetachedHead(w io.Writer, branch, defBranch string, execOpts []process.ExecOption) error {
	if branch == defBranch {
		fmt.Fprintf(w, "%s Already on %s, skipping branch deletion.\n",
			yellowStyle.Render("\u26a0"), defBranch)
		return nil
	}

	if err := fetchOrigin(execOpts...); err != nil {
		return fmt.Errorf("fetch origin: %w", err)
	}
	fmt.Fprintf(w, "%s Fetched origin\n", greenStyle.Render("\u2713"))

	if err := checkoutDetach("origin/"+defBranch, execOpts...); err != nil {
		return fmt.Errorf("checkout origin/%s: %w", defBranch, err)
	}
	fmt.Fprintf(w, "%s Returned to detached HEAD\n", greenStyle.Render("\u2713"))

	if hasLocalBranch(branch) {
		if err := deleteLocalBranch(branch, execOpts...); err != nil {
			fmt.Fprintf(w, "%s Could not delete local branch: %v\n",
				yellowStyle.Render("\u26a0"), err)
		} else {
			fmt.Fprintf(w, "%s Deleted local branch\n", greenStyle.Render("\u2713"))
		}
	}
	return nil
}
