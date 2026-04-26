package closecmd

import (
	"bytes"
	"errors"
	"io"
	"strings"
	"sync"
	"testing"

	"devora/internal/gh"
	"devora/internal/process"
	"devora/internal/tasktracker"
)

// --- Fakes ---

// fakeTracker records calls from closecmd.Run and can be programmed to return
// specific values or errors.
type fakeTracker struct {
	mu sync.Mutex

	// Programmable return values / errors.
	parseTaskURLReturn string
	completeTaskErr    error

	// Recorded calls.
	parseTaskURLCalls []string
	completeTaskCalls []string
}

func (f *fakeTracker) Provider() string                              { return "fake" }
func (f *fakeTracker) WhoAmI() (string, error)                       { return "", nil }
func (f *fakeTracker) CreateTask(tasktracker.CreateTaskRequest) (tasktracker.Task, error) {
	return tasktracker.Task{}, nil
}
func (f *fakeTracker) TaskURL(string) string      { return "" }
func (f *fakeTracker) PRBodyPrefix(string) string { return "" }

func (f *fakeTracker) ParseTaskURL(url string) string {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.parseTaskURLCalls = append(f.parseTaskURLCalls, url)
	return f.parseTaskURLReturn
}

func (f *fakeTracker) CompleteTask(id string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.completeTaskCalls = append(f.completeTaskCalls, id)
	return f.completeTaskErr
}

// --- Stub harness ---

// stubCloseDeps saves and restores ALL package-level function vars so tests
// don't leak state. Call via t.Cleanup.
func stubCloseDeps(t *testing.T) {
	t.Helper()

	origGetCurrentBranchOrDetached := getCurrentBranchOrDetached
	origIsProtectedBranch := isProtectedBranch
	origDefaultBranchName := defaultBranchName
	origGetBranchConfig := getBranchConfig
	origHasRemoteBranch := hasRemoteBranch
	origHasLocalBranch := hasLocalBranch
	origDeleteRemoteBranch := deleteRemoteBranch
	origDeleteLocalBranch := deleteLocalBranch
	origFetchOrigin := fetchOrigin
	origCheckoutDetach := checkoutDetach
	origGetGHPRForBranch := getGHPRForBranch
	origNewTracker := newTracker
	origReadLine := readLine

	t.Cleanup(func() {
		getCurrentBranchOrDetached = origGetCurrentBranchOrDetached
		isProtectedBranch = origIsProtectedBranch
		defaultBranchName = origDefaultBranchName
		getBranchConfig = origGetBranchConfig
		hasRemoteBranch = origHasRemoteBranch
		hasLocalBranch = origHasLocalBranch
		deleteRemoteBranch = origDeleteRemoteBranch
		deleteLocalBranch = origDeleteLocalBranch
		fetchOrigin = origFetchOrigin
		checkoutDetach = origCheckoutDetach
		getGHPRForBranch = origGetGHPRForBranch
		newTracker = origNewTracker
		readLine = origReadLine
	})

	// Safe defaults — tests override only what they care about.
	getCurrentBranchOrDetached = func() (string, error) { return "feature/x", nil }
	isProtectedBranch = func(string) bool { return false }
	defaultBranchName = func() (string, error) { return "main", nil }
	getBranchConfig = func(string, string) (string, error) { return "", nil }
	hasRemoteBranch = func(string) bool { return true }
	hasLocalBranch = func(string) bool { return true }
	deleteRemoteBranch = func(string, ...process.ExecOption) error { return nil }
	deleteLocalBranch = func(string, ...process.ExecOption) error { return nil }
	fetchOrigin = func(...process.ExecOption) error { return nil }
	checkoutDetach = func(string, ...process.ExecOption) error { return nil }
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) { return nil, nil }
	newTracker = func() (tasktracker.Tracker, error) { return nil, nil }
	readLine = func(io.Reader) (string, error) { return "\n", nil }
}

// --- Detached HEAD ---

func TestRun_DetachedHead_ReturnsErrDetached(t *testing.T) {
	stubCloseDeps(t)
	getCurrentBranchOrDetached = func() (string, error) { return "", nil }

	var buf bytes.Buffer
	err := Run(&buf, Options{})
	if !errors.Is(err, ErrDetached) {
		t.Fatalf("expected ErrDetached, got %v", err)
	}
}

// --- Protected branch ---

func TestRun_ProtectedBranch_ReturnsWrappedErrProtectedBranch(t *testing.T) {
	stubCloseDeps(t)
	getCurrentBranchOrDetached = func() (string, error) { return "main", nil }
	isProtectedBranch = func(b string) bool { return b == "main" }

	var buf bytes.Buffer
	err := Run(&buf, Options{})
	if !errors.Is(err, ErrProtectedBranch) {
		t.Fatalf("expected ErrProtectedBranch, got %v", err)
	}
	if !strings.Contains(err.Error(), "main") {
		t.Fatalf("expected error message to contain %q, got %q", "main", err.Error())
	}
}

// --- --task-url with no tracker ---

func TestRun_TaskURLWithNoTracker_ReturnsErrNoTrackerForURL(t *testing.T) {
	stubCloseDeps(t)
	newTracker = func() (tasktracker.Tracker, error) { return nil, nil }

	var buf bytes.Buffer
	err := Run(&buf, Options{TaskURL: "https://example.com/task/1"})
	if !errors.Is(err, ErrNoTrackerForURL) {
		t.Fatalf("expected ErrNoTrackerForURL, got %v", err)
	}
}

// --- --task-url with tracker ---

func TestRun_TaskURLWithTracker_UsesParsedID(t *testing.T) {
	stubCloseDeps(t)
	tracker := &fakeTracker{parseTaskURLReturn: "abc123"}
	newTracker = func() (tasktracker.Tracker, error) { return tracker, nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{TaskURL: "https://example.com/task/1", Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tracker.parseTaskURLCalls) != 1 || tracker.parseTaskURLCalls[0] != "https://example.com/task/1" {
		t.Fatalf("expected ParseTaskURL called with URL, got %v", tracker.parseTaskURLCalls)
	}
	if len(tracker.completeTaskCalls) != 1 || tracker.completeTaskCalls[0] != "abc123" {
		t.Fatalf("expected CompleteTask(%q), got %v", "abc123", tracker.completeTaskCalls)
	}
}

// --- Git config task-id ---

func TestRun_GitConfigTaskID_CallsCompleteTask(t *testing.T) {
	stubCloseDeps(t)
	tracker := &fakeTracker{}
	newTracker = func() (tasktracker.Tracker, error) { return tracker, nil }
	getBranchConfig = func(branch, key string) (string, error) {
		if key == "task-id" {
			return "xyz", nil
		}
		return "", nil
	}

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tracker.completeTaskCalls) != 1 || tracker.completeTaskCalls[0] != "xyz" {
		t.Fatalf("expected CompleteTask(%q), got %v", "xyz", tracker.completeTaskCalls)
	}
}

// --- Missing task-id, no flag ---

func TestRun_MissingTaskID_PrintsWarning_NoCompleteTaskCall(t *testing.T) {
	stubCloseDeps(t)
	tracker := &fakeTracker{}
	newTracker = func() (tasktracker.Tracker, error) { return tracker, nil }
	getBranchConfig = func(string, string) (string, error) { return "", nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tracker.completeTaskCalls) != 0 {
		t.Fatalf("expected no CompleteTask calls, got %v", tracker.completeTaskCalls)
	}
	if !strings.Contains(buf.String(), "No task ID") {
		t.Fatalf("expected output to contain no-task-id warning, got %q", buf.String())
	}
}

// --- SkipTracker ---

func TestRun_SkipTracker_BypassesTracker(t *testing.T) {
	stubCloseDeps(t)
	tracker := &fakeTracker{}
	newTracker = func() (tasktracker.Tracker, error) { return tracker, nil }
	getBranchConfig = func(string, string) (string, error) { return "xyz", nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{SkipTracker: true, Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tracker.completeTaskCalls) != 0 {
		t.Fatalf("expected no CompleteTask calls when SkipTracker, got %v", tracker.completeTaskCalls)
	}
}

// --- Open PR + force ---

func TestRun_OpenPRWithForce_NoPromptProceeds(t *testing.T) {
	stubCloseDeps(t)
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return &gh.PRSummary{Number: 7, URL: "https://gh/pr/7", State: "OPEN"}, nil
	}
	readLineCalled := false
	readLine = func(io.Reader) (string, error) {
		readLineCalled = true
		return "\n", nil
	}

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if readLineCalled {
		t.Fatalf("expected no prompt under Force, but readLine was called")
	}
}

// --- Open PR + no force + yes ---

func TestRun_OpenPRNoForce_Yes_Proceeds(t *testing.T) {
	stubCloseDeps(t)
	tracker := &fakeTracker{}
	newTracker = func() (tasktracker.Tracker, error) { return tracker, nil }
	getBranchConfig = func(string, string) (string, error) { return "xyz", nil }
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return &gh.PRSummary{Number: 7, URL: "https://gh/pr/7", State: "OPEN"}, nil
	}
	readLine = func(io.Reader) (string, error) { return "y\n", nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tracker.completeTaskCalls) != 1 {
		t.Fatalf("expected CompleteTask called once, got %v", tracker.completeTaskCalls)
	}
}

// --- Open PR + no force + no ---

func TestRun_OpenPRNoForce_No_AbortsWithErrAborted(t *testing.T) {
	stubCloseDeps(t)
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return &gh.PRSummary{Number: 7, URL: "https://gh/pr/7", State: "OPEN"}, nil
	}
	readLine = func(io.Reader) (string, error) { return "\n", nil }

	var buf bytes.Buffer
	err := Run(&buf, Options{})
	if !errors.Is(err, ErrAborted) {
		t.Fatalf("expected ErrAborted, got %v", err)
	}
	if !strings.Contains(buf.String(), "Aborted") {
		t.Fatalf("expected %q in output, got %q", "Aborted", buf.String())
	}
}

// --- PR check errors ---

func TestRun_PRCheckError_PrintsWarning_Proceeds(t *testing.T) {
	stubCloseDeps(t)
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return nil, errors.New("gh exploded")
	}

	var buf bytes.Buffer
	if err := Run(&buf, Options{}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(buf.String(), "Could not check PR status") {
		t.Fatalf("expected PR status warning, got %q", buf.String())
	}
}

// --- CompleteTask fails ---

func TestRun_CompleteTaskFails_NonFatal(t *testing.T) {
	stubCloseDeps(t)
	tracker := &fakeTracker{completeTaskErr: errors.New("boom")}
	newTracker = func() (tasktracker.Tracker, error) { return tracker, nil }
	getBranchConfig = func(string, string) (string, error) { return "xyz", nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("expected nil, got %v", err)
	}
	if !strings.Contains(buf.String(), "Could not mark task as complete") {
		t.Fatalf("expected completion warning, got %q", buf.String())
	}
}

// --- DeleteRemoteBranch fails ---

func TestRun_DeleteRemoteBranchFails_NonFatal(t *testing.T) {
	stubCloseDeps(t)
	deleteRemoteBranch = func(string, ...process.ExecOption) error { return errors.New("push failed") }

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("expected nil, got %v", err)
	}
	if !strings.Contains(buf.String(), "Could not delete remote branch") {
		t.Fatalf("expected remote-delete warning, got %q", buf.String())
	}
}

// --- Remote branch already gone ---

func TestRun_RemoteBranchGone_PrintsAlreadyDeleted(t *testing.T) {
	stubCloseDeps(t)
	called := false
	hasRemoteBranch = func(string) bool { return false }
	deleteRemoteBranch = func(string, ...process.ExecOption) error {
		called = true
		return nil
	}

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("expected nil, got %v", err)
	}
	if called {
		t.Fatalf("expected DeleteRemoteBranch not to be called when remote is gone")
	}
	if !strings.Contains(buf.String(), "Remote branch already deleted") {
		t.Fatalf("expected 'Remote branch already deleted' in output, got %q", buf.String())
	}
}

// --- Same branch as default ---

func TestRun_BranchIsDefault_SkipsFetchCheckoutDelete(t *testing.T) {
	stubCloseDeps(t)
	getCurrentBranchOrDetached = func() (string, error) { return "main", nil }
	isProtectedBranch = func(string) bool { return false } // pretend main not protected for this test
	defaultBranchName = func() (string, error) { return "main", nil }

	fetchCalled := false
	checkoutCalled := false
	deleteLocalCalled := false
	fetchOrigin = func(...process.ExecOption) error { fetchCalled = true; return nil }
	checkoutDetach = func(string, ...process.ExecOption) error { checkoutCalled = true; return nil }
	deleteLocalBranch = func(string, ...process.ExecOption) error { deleteLocalCalled = true; return nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if fetchCalled || checkoutCalled || deleteLocalCalled {
		t.Fatalf("expected no fetch/checkout/delete-local calls when on default branch; got fetch=%v checkout=%v delete=%v",
			fetchCalled, checkoutCalled, deleteLocalCalled)
	}
	if !strings.Contains(buf.String(), "Already on") {
		t.Fatalf("expected 'Already on' warning, got %q", buf.String())
	}
}

// --- Fetch fails ---

func TestRun_FetchFails_HardFail(t *testing.T) {
	stubCloseDeps(t)
	fetchOrigin = func(...process.ExecOption) error { return errors.New("fetch boom") }

	var buf bytes.Buffer
	err := Run(&buf, Options{Force: true})
	if err == nil {
		t.Fatalf("expected error, got nil")
	}
	if !strings.Contains(err.Error(), "fetch boom") {
		t.Fatalf("expected wrapped fetch error, got %v", err)
	}
}

// --- Checkout fails ---

func TestRun_CheckoutFails_HardFail(t *testing.T) {
	stubCloseDeps(t)
	checkoutDetach = func(string, ...process.ExecOption) error { return errors.New("checkout boom") }

	var buf bytes.Buffer
	err := Run(&buf, Options{Force: true})
	if err == nil {
		t.Fatalf("expected error, got nil")
	}
	if !strings.Contains(err.Error(), "checkout boom") {
		t.Fatalf("expected wrapped checkout error, got %v", err)
	}
}

// --- DeleteLocalBranch fails ---

func TestRun_DeleteLocalBranchFails_NonFatal(t *testing.T) {
	stubCloseDeps(t)
	deleteLocalBranch = func(string, ...process.ExecOption) error { return errors.New("local delete boom") }

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("expected nil, got %v", err)
	}
}

// --- confirm helper ---

func TestConfirm_YesResponses(t *testing.T) {
	stubCloseDeps(t)
	cases := []string{"y\n", "Y\n", "yes\n", "YES\n", "  yes  \n", "Yes\n"}
	for _, c := range cases {
		c := c
		t.Run(strings.TrimSpace(c), func(t *testing.T) {
			readLine = func(io.Reader) (string, error) { return c, nil }
			var buf bytes.Buffer
			got, err := confirm(&buf, "confirm?")
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if !got {
				t.Fatalf("expected true for %q", c)
			}
		})
	}
}

func TestConfirm_NoResponses(t *testing.T) {
	stubCloseDeps(t)
	cases := []string{"\n", "n\n", "no\n", "anything else\n", "  \n"}
	for _, c := range cases {
		c := c
		t.Run(strings.TrimSpace(c), func(t *testing.T) {
			readLine = func(io.Reader) (string, error) { return c, nil }
			var buf bytes.Buffer
			got, err := confirm(&buf, "confirm?")
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if got {
				t.Fatalf("expected false for %q", c)
			}
		})
	}
}

func TestConfirm_StdinReadError_Propagated(t *testing.T) {
	stubCloseDeps(t)
	readLine = func(io.Reader) (string, error) { return "", errors.New("stdin blew up") }
	var buf bytes.Buffer
	_, err := confirm(&buf, "confirm?")
	if err == nil {
		t.Fatalf("expected error, got nil")
	}
	if !strings.Contains(err.Error(), "stdin blew up") {
		t.Fatalf("expected wrapped stdin error, got %v", err)
	}
}

// --- PR open prompt message content ---

func TestRun_OpenPR_PromptContainsPRInfo(t *testing.T) {
	stubCloseDeps(t)
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return &gh.PRSummary{Number: 42, URL: "https://gh/pr/42", State: "OPEN"}, nil
	}
	readLine = func(io.Reader) (string, error) { return "\n", nil }

	var buf bytes.Buffer
	_ = Run(&buf, Options{})
	out := buf.String()
	if !strings.Contains(out, "#42") || !strings.Contains(out, "https://gh/pr/42") {
		t.Fatalf("expected PR number/URL in prompt, got %q", out)
	}
}

// --- Verbosity modes ---

func TestRun_NormalMode_OutputHasProgressAndClosed(t *testing.T) {
	stubCloseDeps(t)

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	out := buf.String()
	// Expected progress lines in Normal mode.
	for _, want := range []string{"Deleted remote branch", "Fetched origin", "Returned to detached HEAD", "Closed"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected Normal output to contain %q, got: %q", want, out)
		}
	}
}

func TestRun_QuietMode_OutputIsOnlyClosed(t *testing.T) {
	stubCloseDeps(t)

	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true, Quiet: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	out := buf.String()
	// Only the final ✓ Closed line should be on stdout.
	if !strings.Contains(out, "Closed") {
		t.Fatalf("expected Quiet output to contain 'Closed', got: %q", out)
	}
	for _, unwanted := range []string{"Fetched origin", "Returned to detached HEAD", "Deleted remote branch"} {
		if strings.Contains(out, unwanted) {
			t.Fatalf("expected Quiet output to NOT contain %q, got: %q", unwanted, out)
		}
	}
}

func TestRun_VerbosePassesNoExecOpts_NormalPassesWithSilent(t *testing.T) {
	stubCloseDeps(t)
	var gotVerbOpts, gotNormalOpts int
	fetchOrigin = func(opts ...process.ExecOption) error {
		// Overwritten per subtest; default just counts.
		return nil
	}

	// Verbose
	fetchOrigin = func(opts ...process.ExecOption) error {
		gotVerbOpts = len(opts)
		return nil
	}
	var buf bytes.Buffer
	if err := Run(&buf, Options{Force: true, Verbose: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotVerbOpts != 0 {
		t.Fatalf("expected Verbose to pass 0 exec opts, got %d", gotVerbOpts)
	}

	// Normal
	stubCloseDeps(t)
	fetchOrigin = func(opts ...process.ExecOption) error {
		gotNormalOpts = len(opts)
		return nil
	}
	buf.Reset()
	if err := Run(&buf, Options{Force: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotNormalOpts == 0 {
		t.Fatal("expected Normal to pass WithSilent exec opts")
	}
}

func TestRun_PromptPrintsEvenInQuietMode(t *testing.T) {
	stubCloseDeps(t)
	getGHPRForBranch = func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return &gh.PRSummary{State: "OPEN", Number: 7, URL: "https://example/pr/7"}, nil
	}
	readLine = func(io.Reader) (string, error) { return "y\n", nil }

	var buf bytes.Buffer
	if err := Run(&buf, Options{Quiet: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(buf.String(), "is still open") {
		t.Fatalf("expected open-PR warning printed even in Quiet mode, got: %q", buf.String())
	}
	if !strings.Contains(buf.String(), "Are you sure") {
		t.Fatalf("expected prompt text printed even in Quiet mode, got: %q", buf.String())
	}
}
