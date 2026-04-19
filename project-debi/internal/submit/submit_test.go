package submit

import (
	"bytes"
	"encoding/json"
	"errors"
	"strings"
	"testing"

	"devora/internal/gh"
	"devora/internal/process"
	"devora/internal/tasktracker"
)

// --- Fake Tracker ---

// fakeTracker is a minimal Tracker impl for tests. It records every call
// so assertions can verify that submit wired inputs through correctly.
type fakeTracker struct {
	whoAmIReturn string
	whoAmIErr    error
	whoAmICalls  int

	createTaskReturn tasktracker.Task
	createTaskErr    error
	createTaskCalls  []tasktracker.CreateTaskRequest
}

func (f *fakeTracker) Provider() string { return "asana" }

func (f *fakeTracker) WhoAmI() (string, error) {
	f.whoAmICalls++
	return f.whoAmIReturn, f.whoAmIErr
}

func (f *fakeTracker) CreateTask(req tasktracker.CreateTaskRequest) (tasktracker.Task, error) {
	f.createTaskCalls = append(f.createTaskCalls, req)
	return f.createTaskReturn, f.createTaskErr
}

func (f *fakeTracker) CompleteTask(string) error  { return nil }
func (f *fakeTracker) ParseTaskURL(string) string { return "" }

func (f *fakeTracker) TaskURL(id string) string      { return "https://example/task/" + id }
func (f *fakeTracker) PRBodyPrefix(id string) string { return "asana: " + f.TaskURL(id) }

// --- Test harness: save + restore every stubbable var ---

type stubs struct {
	// recorded inputs — populated by default stubs so each test can assert.
	addCommitMessages    []string
	generatedBranchInput struct {
		prefix, msg string
	}
	createdBranches  []string
	pushedBranches   []string
	setBranchConfigs []struct{ branch, key, value string }
	createPRCalls    []struct {
		title, body string
		draft       bool
	}
	autoMergeCalls       []string
	browserCalls         []string
	getRepoCalls         int
	getGHPRViewCalls     []string
	newTrackerCalls      int
	addCommitOptCount    int

	// control values — tests overwrite these before calling Run.
	currentBranch        string
	currentBranchErr     error
	addCommitErr         error
	generateBranchReturn string
	createBranchErr      error
	setBranchConfigErr   error
	pushErr              error
	repoInfo             gh.RepoInfo
	repoInfoErr          error
	createPRURL          string
	createPRErr          error
	autoMergeErr         error
	getGHPRViewReturn    gh.PRSummary
	getGHPRViewErr       error
	newTrackerReturn     tasktracker.Tracker
	newTrackerErr        error
	branchPrefix         string
	openBrowserErr       error
}

// stubSubmitDeps replaces every package-level dependency var with a
// recording/controllable stub for the duration of the test. Returns a *stubs
// that tests mutate to configure return values and inspect recorded calls.
func stubSubmitDeps(t *testing.T) *stubs {
	t.Helper()

	orig := struct {
		getCurrentBranchOrDetached func() (string, error)
		addAllAndCommit            func(string, ...process.ExecOption) error
		generateBranchName         func(string, string) string
		createAndCheckoutBranch    func(string, ...process.ExecOption) error
		setBranchConfig            func(string, string, string, ...process.ExecOption) error
		pushSetUpstream            func(string, ...process.ExecOption) error
		getGHRepo                  func() (gh.RepoInfo, error)
		createGHPR                 func(string, string, bool) (string, error)
		enableGHAutoMerge          func(string, ...process.ExecOption) error
		newTracker                 func() (tasktracker.Tracker, error)
		getBranchPrefix            func(string) string
		openBrowser                func(string) error
		getGHPRView                func(string) (gh.PRSummary, error)
	}{
		getCurrentBranchOrDetached: getCurrentBranchOrDetached,
		addAllAndCommit:            addAllAndCommit,
		generateBranchName:         generateBranchName,
		createAndCheckoutBranch:    createAndCheckoutBranch,
		setBranchConfig:            setBranchConfig,
		pushSetUpstream:            pushSetUpstream,
		getGHRepo:                  getGHRepo,
		createGHPR:                 createGHPR,
		enableGHAutoMerge:          enableGHAutoMerge,
		newTracker:                 newTracker,
		getBranchPrefix:            getBranchPrefix,
		openBrowser:                openBrowser,
		getGHPRView:                getGHPRView,
	}

	t.Cleanup(func() {
		getCurrentBranchOrDetached = orig.getCurrentBranchOrDetached
		addAllAndCommit = orig.addAllAndCommit
		generateBranchName = orig.generateBranchName
		createAndCheckoutBranch = orig.createAndCheckoutBranch
		setBranchConfig = orig.setBranchConfig
		pushSetUpstream = orig.pushSetUpstream
		getGHRepo = orig.getGHRepo
		createGHPR = orig.createGHPR
		enableGHAutoMerge = orig.enableGHAutoMerge
		newTracker = orig.newTracker
		getBranchPrefix = orig.getBranchPrefix
		openBrowser = orig.openBrowser
		getGHPRView = orig.getGHPRView
	})

	s := &stubs{
		// Successful defaults: detached HEAD, everything returns nil error.
		currentBranch:        "",
		generateBranchReturn: "feature-msg",
		createPRURL:          "https://github.com/owner/repo/pull/42",
		repoInfo:             gh.RepoInfo{DefaultBranch: "main"},
		getGHPRViewReturn:    gh.PRSummary{Number: 42, Title: "Msg"},
		branchPrefix:         "feature",
	}

	getCurrentBranchOrDetached = func() (string, error) {
		return s.currentBranch, s.currentBranchErr
	}
	addAllAndCommit = func(message string, opts ...process.ExecOption) error {
		s.addCommitMessages = append(s.addCommitMessages, message)
		s.addCommitOptCount = len(opts)
		return s.addCommitErr
	}
	generateBranchName = func(prefix, msg string) string {
		s.generatedBranchInput.prefix = prefix
		s.generatedBranchInput.msg = msg
		return s.generateBranchReturn
	}
	createAndCheckoutBranch = func(name string, opts ...process.ExecOption) error {
		s.createdBranches = append(s.createdBranches, name)
		return s.createBranchErr
	}
	setBranchConfig = func(branch, key, value string, opts ...process.ExecOption) error {
		s.setBranchConfigs = append(s.setBranchConfigs, struct{ branch, key, value string }{branch, key, value})
		return s.setBranchConfigErr
	}
	pushSetUpstream = func(branch string, opts ...process.ExecOption) error {
		s.pushedBranches = append(s.pushedBranches, branch)
		return s.pushErr
	}
	getGHRepo = func() (gh.RepoInfo, error) {
		s.getRepoCalls++
		return s.repoInfo, s.repoInfoErr
	}
	createGHPR = func(title, body string, draft bool) (string, error) {
		s.createPRCalls = append(s.createPRCalls, struct {
			title, body string
			draft       bool
		}{title, body, draft})
		return s.createPRURL, s.createPRErr
	}
	enableGHAutoMerge = func(url string, opts ...process.ExecOption) error {
		s.autoMergeCalls = append(s.autoMergeCalls, url)
		return s.autoMergeErr
	}
	newTracker = func() (tasktracker.Tracker, error) {
		s.newTrackerCalls++
		return s.newTrackerReturn, s.newTrackerErr
	}
	getBranchPrefix = func(fallback string) string {
		if s.branchPrefix == "" {
			return fallback
		}
		return s.branchPrefix
	}
	openBrowser = func(url string) error {
		s.browserCalls = append(s.browserCalls, url)
		return s.openBrowserErr
	}
	getGHPRView = func(url string) (gh.PRSummary, error) {
		s.getGHPRViewCalls = append(s.getGHPRViewCalls, url)
		return s.getGHPRViewReturn, s.getGHPRViewErr
	}

	return s
}

// --- Tests ---

func TestRun_MissingMessage_ReturnsError(t *testing.T) {
	stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: ""})
	if err == nil {
		t.Fatal("expected error for missing message")
	}
	if !strings.Contains(err.Error(), "--message") {
		t.Fatalf("expected error to mention --message, got: %v", err)
	}
}

func TestRun_NotDetachedHead_ReturnsErrNotDetached(t *testing.T) {
	s := stubSubmitDeps(t)
	s.currentBranch = "main"

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix bug"})
	if err == nil {
		t.Fatal("expected error when not detached")
	}
	if !errors.Is(err, ErrNotDetached) {
		t.Fatalf("expected errors.Is(err, ErrNotDetached) to be true, got: %v", err)
	}
	if !strings.Contains(err.Error(), "main") {
		t.Fatalf("expected error to mention branch 'main', got: %v", err)
	}
}

func TestRun_GetBranchErr_Propagates(t *testing.T) {
	s := stubSubmitDeps(t)
	s.currentBranchErr = errors.New("not in a repo")

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err == nil {
		t.Fatal("expected error propagated from git")
	}
	if errors.Is(err, ErrNotDetached) {
		t.Fatalf("expected non-sentinel error, got: %v", err)
	}
}

func TestRun_NoTracker_SkipsTaskCreation(t *testing.T) {
	s := stubSubmitDeps(t)
	// newTracker returns (nil, nil) by default: no tracker configured.

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix bug"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(s.createPRCalls) != 1 {
		t.Fatalf("expected 1 CreatePR call, got %d", len(s.createPRCalls))
	}
	// With no tracker and no description, body should be empty.
	if s.createPRCalls[0].body != "" {
		t.Fatalf("expected empty PR body, got %q", s.createPRCalls[0].body)
	}
	// No task-id branch config should be written.
	if len(s.setBranchConfigs) != 0 {
		t.Fatalf("expected 0 branch-config writes, got %d: %+v", len(s.setBranchConfigs), s.setBranchConfigs)
	}
	if strings.Contains(buf.String(), "Task created") {
		t.Fatalf("output should not mention Task created when no tracker, got:\n%s", buf.String())
	}
}

func TestRun_NoTracker_DescriptionOnlyBody(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix bug", Description: "Hello body"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.createPRCalls[0].body != "Hello body" {
		t.Fatalf("expected body to equal description, got %q", s.createPRCalls[0].body)
	}
}

func TestRun_WithTracker_CreatesTaskAndSetsBranchConfig(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		whoAmIReturn:     "user-123",
		createTaskReturn: tasktracker.Task{ID: "task-789", URL: "https://app.asana.com/task/789"},
	}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix bug", Description: "details"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if tracker.whoAmICalls != 1 {
		t.Fatalf("expected WhoAmI called once, got %d", tracker.whoAmICalls)
	}
	if len(tracker.createTaskCalls) != 1 {
		t.Fatalf("expected 1 CreateTask call, got %d", len(tracker.createTaskCalls))
	}
	ct := tracker.createTaskCalls[0]
	if ct.Title != "Fix bug" {
		t.Fatalf("expected title 'Fix bug', got %q", ct.Title)
	}
	if ct.AssigneeGID != "user-123" {
		t.Fatalf("expected assignee 'user-123', got %q", ct.AssigneeGID)
	}

	// Branch config should be set with "task-id" key.
	if len(s.setBranchConfigs) != 1 {
		t.Fatalf("expected 1 SetBranchConfig call, got %d: %+v", len(s.setBranchConfigs), s.setBranchConfigs)
	}
	bc := s.setBranchConfigs[0]
	if bc.key != "task-id" {
		t.Fatalf("expected key 'task-id', got %q", bc.key)
	}
	if bc.value != "task-789" {
		t.Fatalf("expected value 'task-789', got %q", bc.value)
	}
	if bc.branch != s.generateBranchReturn {
		t.Fatalf("expected branch %q, got %q", s.generateBranchReturn, bc.branch)
	}

	// PR body should start with the tracker prefix, followed by description.
	body := s.createPRCalls[0].body
	if !strings.HasPrefix(body, "asana: ") {
		t.Fatalf("expected body to start with 'asana: ', got %q", body)
	}
	if !strings.Contains(body, "details") {
		t.Fatalf("expected body to contain description, got %q", body)
	}
	if !strings.Contains(body, "\n\ndetails") {
		t.Fatalf("expected double-newline separator before description, got %q", body)
	}

	if !strings.Contains(buf.String(), "Task created") {
		t.Fatalf("expected output to mention 'Task created', got:\n%s", buf.String())
	}
}

func TestRun_WithTracker_NoDescription_BodyIsPrefixOnly(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		createTaskReturn: tasktracker.Task{ID: "t1", URL: "https://example/t1"},
	}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	body := s.createPRCalls[0].body
	if !strings.HasPrefix(body, "asana: ") {
		t.Fatalf("expected body to start with tracker prefix, got %q", body)
	}
	if strings.Contains(body, "\n\n") {
		t.Fatalf("expected no trailing double-newline, got %q", body)
	}
}

func TestRun_SkipTracker_IgnoresConfiguredTracker(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", SkipTracker: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tracker.whoAmICalls != 0 {
		t.Fatalf("expected WhoAmI not called, got %d", tracker.whoAmICalls)
	}
	if len(tracker.createTaskCalls) != 0 {
		t.Fatalf("expected CreateTask not called, got %d", len(tracker.createTaskCalls))
	}
	if len(s.setBranchConfigs) != 0 {
		t.Fatalf("expected 0 branch-config writes, got %d", len(s.setBranchConfigs))
	}
	if s.newTrackerCalls != 0 {
		t.Fatalf("expected newTracker not called when SkipTracker, got %d", s.newTrackerCalls)
	}
}

func TestRun_NewTrackerError_Propagates(t *testing.T) {
	s := stubSubmitDeps(t)
	s.newTrackerErr = errors.New("missing workspace-id")

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err == nil {
		t.Fatal("expected error propagated from newTracker")
	}
	if !strings.Contains(err.Error(), "missing workspace-id") {
		t.Fatalf("expected wrapped error, got: %v", err)
	}
}

func TestRun_DraftFlag_ForwardedToCreatePR(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", Draft: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !s.createPRCalls[0].draft {
		t.Fatalf("expected CreatePR called with draft=true, got draft=%v", s.createPRCalls[0].draft)
	}
}

func TestRun_Blocked_SkipsAutoMerge(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", Blocked: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(s.autoMergeCalls) != 0 {
		t.Fatalf("expected auto-merge not called, got %d calls", len(s.autoMergeCalls))
	}
}

func TestRun_NotBlocked_EnablesAutoMerge(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(s.autoMergeCalls) != 1 {
		t.Fatalf("expected 1 auto-merge call, got %d", len(s.autoMergeCalls))
	}
	if s.autoMergeCalls[0] != s.createPRURL {
		t.Fatalf("expected auto-merge on %q, got %q", s.createPRURL, s.autoMergeCalls[0])
	}
}

// stubStderr replaces the package-level stderr sink for the duration of the
// test and returns a buffer that captures everything written to it.
func stubStderr(t *testing.T) *bytes.Buffer {
	t.Helper()
	orig := stderr
	var buf bytes.Buffer
	stderr = &buf
	t.Cleanup(func() { stderr = orig })
	return &buf
}

func TestRun_PRAlreadyExists_PrintsAndReturnsWrappedError(t *testing.T) {
	s := stubSubmitDeps(t)
	errBuf := stubStderr(t)
	s.createPRErr = &gh.PRAlreadyExistsError{
		Branch:      "feature-msg",
		ExistingURL: "https://github.com/owner/repo/pull/7",
	}

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err == nil {
		t.Fatal("expected error from PR already exists")
	}
	var exists *gh.PRAlreadyExistsError
	if !errors.As(err, &exists) {
		t.Fatalf("expected errors.As *gh.PRAlreadyExistsError, got: %v", err)
	}
	if !strings.Contains(errBuf.String(), "https://github.com/owner/repo/pull/7") {
		t.Fatalf("expected existing URL printed to stderr, got stderr:\n%s", errBuf.String())
	}
	// Auto-merge must not be attempted when PR creation failed.
	if len(s.autoMergeCalls) != 0 {
		t.Fatalf("expected 0 auto-merge calls on PR-exists error, got %d", len(s.autoMergeCalls))
	}
}

func TestRun_PRAlreadyExists_EmptyURL_NoLinkPrinted(t *testing.T) {
	s := stubSubmitDeps(t)
	errBuf := stubStderr(t)
	s.createPRErr = &gh.PRAlreadyExistsError{Branch: "feature-msg"}

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err == nil {
		t.Fatal("expected error")
	}
	// Without a URL we still print the warning line to stderr, but nothing
	// claims an http link exists.
	combined := buf.String() + errBuf.String()
	if strings.Contains(combined, "http://") || strings.Contains(combined, "https://") {
		t.Fatalf("expected no URL in output, got stdout:\n%s\nstderr:\n%s", buf.String(), errBuf.String())
	}
}

func TestRun_JSONMode_PRAlreadyExists_StdoutEmpty_StderrHasWarning(t *testing.T) {
	s := stubSubmitDeps(t)
	errBuf := stubStderr(t)
	s.createPRErr = &gh.PRAlreadyExistsError{
		Branch:      "feature-msg",
		ExistingURL: "https://github.com/owner/repo/pull/7",
	}

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", JSONOutput: true})
	if err == nil {
		t.Fatal("expected error from PR already exists")
	}
	var exists *gh.PRAlreadyExistsError
	if !errors.As(err, &exists) {
		t.Fatalf("expected errors.As *gh.PRAlreadyExistsError, got: %v", err)
	}
	// In JSON mode w (stdout) must contain no output when PR creation failed.
	if buf.Len() != 0 {
		t.Fatalf("expected empty stdout in JSON mode on PR-exists error, got:\n%q", buf.String())
	}
	// The warning must be routed to stderr.
	if !strings.Contains(errBuf.String(), "https://github.com/owner/repo/pull/7") {
		t.Fatalf("expected existing URL on stderr, got:\n%s", errBuf.String())
	}
	if !strings.Contains(errBuf.String(), "PR already exists") {
		t.Fatalf("expected 'PR already exists' warning on stderr, got:\n%s", errBuf.String())
	}
}

func TestRun_AutoMergeFails_WarnsAndContinues(t *testing.T) {
	s := stubSubmitDeps(t)
	s.autoMergeErr = errors.New("no write access")

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("expected nil error (auto-merge is non-fatal), got: %v", err)
	}
	out := buf.String()
	if !strings.Contains(out, "auto-merge") {
		t.Fatalf("expected output to mention auto-merge warning, got:\n%s", out)
	}
	if !strings.Contains(out, "no write access") {
		t.Fatalf("expected underlying error included in warning, got:\n%s", out)
	}
}

func TestRun_JSONOutput_HappyPath(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		createTaskReturn: tasktracker.Task{ID: "t1", URL: "https://example/t1"},
	}
	s.newTrackerReturn = tracker
	s.generateBranchReturn = "feature-fix-bug"
	s.getGHPRViewReturn = gh.PRSummary{Number: 99, Title: "Fix bug"}
	s.repoInfo = gh.RepoInfo{DefaultBranch: "main"}

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix bug", Draft: true, JSONOutput: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	var out JSONOutput
	if err := json.Unmarshal(buf.Bytes(), &out); err != nil {
		t.Fatalf("expected valid JSON output, got err: %v\n%s", err, buf.String())
	}
	if out.Number != 99 {
		t.Fatalf("expected number 99, got %d", out.Number)
	}
	if out.URL != s.createPRURL {
		t.Fatalf("expected URL %q, got %q", s.createPRURL, out.URL)
	}
	if out.Title != "Fix bug" {
		t.Fatalf("expected title 'Fix bug', got %q", out.Title)
	}
	if out.Branch != "feature-fix-bug" {
		t.Fatalf("expected branch 'feature-fix-bug', got %q", out.Branch)
	}
	if out.BaseBranch != "main" {
		t.Fatalf("expected base_branch 'main', got %q", out.BaseBranch)
	}
	if !out.Draft {
		t.Fatalf("expected draft true, got false")
	}
	if out.TaskURL != "https://example/t1" {
		t.Fatalf("expected task_url 'https://example/t1', got %q", out.TaskURL)
	}
}

func TestRun_JSONOutput_NoTracker_EmptyTaskURL(t *testing.T) {
	s := stubSubmitDeps(t)
	s.getGHPRViewReturn = gh.PRSummary{Number: 5, Title: "Fix"}

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", JSONOutput: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	var out JSONOutput
	if err := json.Unmarshal(buf.Bytes(), &out); err != nil {
		t.Fatalf("expected valid JSON, got: %v", err)
	}
	if out.TaskURL != "" {
		t.Fatalf("expected empty task_url, got %q", out.TaskURL)
	}
}

func TestRun_JSONOutput_GetGHPRViewFails_StillSucceeds(t *testing.T) {
	s := stubSubmitDeps(t)
	s.getGHPRViewErr = errors.New("rate limited")
	s.generateBranchReturn = "feature-x"

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", JSONOutput: true, Draft: true})
	if err != nil {
		t.Fatalf("expected JSON emission to still succeed, got: %v", err)
	}
	// JSON should still parse. Number may be zero; title falls back to message.
	// The emitter writes JSON to w; a warning line may be printed before it.
	// Find the JSON payload: the first '{' onward should parse.
	s2 := buf.String()
	idx := strings.Index(s2, "{")
	if idx < 0 {
		t.Fatalf("expected JSON in output, got:\n%s", s2)
	}
	var out JSONOutput
	if err := json.Unmarshal([]byte(s2[idx:]), &out); err != nil {
		t.Fatalf("expected valid JSON even on fallback, got err: %v\nout:\n%s", err, s2)
	}
	if out.URL != s.createPRURL {
		t.Fatalf("expected URL populated from createPR, got %q", out.URL)
	}
	if out.Branch != "feature-x" {
		t.Fatalf("expected branch 'feature-x', got %q", out.Branch)
	}
	if out.Title != "Fix" {
		t.Fatalf("expected title fallback to opts.Message 'Fix', got %q", out.Title)
	}
	if !out.Draft {
		t.Fatalf("expected draft true, got false")
	}
}

func TestRun_OpenBrowser_CallsOpen(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", OpenBrowser: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(s.browserCalls) != 1 {
		t.Fatalf("expected 1 browser call, got %d", len(s.browserCalls))
	}
	if s.browserCalls[0] != s.createPRURL {
		t.Fatalf("expected browser open for %q, got %q", s.createPRURL, s.browserCalls[0])
	}
}

func TestRun_NoOpenBrowser_NoBrowserCall(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(s.browserCalls) != 0 {
		t.Fatalf("expected 0 browser calls, got %d", len(s.browserCalls))
	}
}

func TestRun_PreFetch_ParallelWhenBothNeeded(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		whoAmIReturn:     "user-id",
		createTaskReturn: tasktracker.Task{ID: "t1", URL: "https://example/t1"},
	}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", JSONOutput: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Both pre-fetch paths should have been exercised.
	if s.getRepoCalls != 1 {
		t.Fatalf("expected getGHRepo called once, got %d", s.getRepoCalls)
	}
	if tracker.whoAmICalls != 1 {
		t.Fatalf("expected WhoAmI called once, got %d", tracker.whoAmICalls)
	}
}

func TestRun_PreFetch_SkippedWhenNeither(t *testing.T) {
	s := stubSubmitDeps(t)
	// No tracker, no JSON: neither pre-fetch is required.

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.getRepoCalls != 0 {
		t.Fatalf("expected getGHRepo not called, got %d", s.getRepoCalls)
	}
}

func TestRun_PreFetch_OnlyTracker_NoRepoCall(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		whoAmIReturn:     "u1",
		createTaskReturn: tasktracker.Task{ID: "t1", URL: "https://example/t1"},
	}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"}) // no JSON
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.getRepoCalls != 0 {
		t.Fatalf("expected getGHRepo not called when no JSON, got %d", s.getRepoCalls)
	}
	if tracker.whoAmICalls != 1 {
		t.Fatalf("expected WhoAmI called once, got %d", tracker.whoAmICalls)
	}
}

func TestRun_PreFetch_OnlyJSON_NoWhoAmICall(t *testing.T) {
	s := stubSubmitDeps(t)
	// No tracker configured.

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix", JSONOutput: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.getRepoCalls != 1 {
		t.Fatalf("expected getGHRepo called once, got %d", s.getRepoCalls)
	}
}

func TestRun_CallsBranchPrefixWithFeatureFallback(t *testing.T) {
	s := stubSubmitDeps(t)
	s.branchPrefix = "" // force fallback path

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.generatedBranchInput.prefix != "feature" {
		t.Fatalf("expected branch prefix fallback 'feature', got %q", s.generatedBranchInput.prefix)
	}
	if s.generatedBranchInput.msg != "Fix" {
		t.Fatalf("expected GenerateBranchName called with message 'Fix', got %q", s.generatedBranchInput.msg)
	}
}

func TestRun_CommitsMessageVerbatim(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix the flaky test"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(s.addCommitMessages) != 1 {
		t.Fatalf("expected 1 commit call, got %d", len(s.addCommitMessages))
	}
	if s.addCommitMessages[0] != "Fix the flaky test" {
		t.Fatalf("expected commit message 'Fix the flaky test', got %q", s.addCommitMessages[0])
	}
}

func TestRun_PushesAndCreatesBranchInOrder(t *testing.T) {
	s := stubSubmitDeps(t)
	s.generateBranchReturn = "feature-fix"

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(s.createdBranches) != 1 || s.createdBranches[0] != "feature-fix" {
		t.Fatalf("expected 1 branch creation for 'feature-fix', got %+v", s.createdBranches)
	}
	if len(s.pushedBranches) != 1 || s.pushedBranches[0] != "feature-fix" {
		t.Fatalf("expected 1 push for 'feature-fix', got %+v", s.pushedBranches)
	}
}

func TestRun_NonJSON_HumanSummaryIncludesURLAndBranch(t *testing.T) {
	s := stubSubmitDeps(t)
	s.generateBranchReturn = "feature-fix"

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	out := buf.String()
	if !strings.Contains(out, s.createPRURL) {
		t.Fatalf("expected PR URL in human output, got:\n%s", out)
	}
	if !strings.Contains(out, "feature-fix") {
		t.Fatalf("expected branch name in human output, got:\n%s", out)
	}
}

func TestRun_NonJSON_WithTracker_PrintsTaskURL(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		createTaskReturn: tasktracker.Task{ID: "t1", URL: "https://example/t1"},
	}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(buf.String(), "https://example/t1") {
		t.Fatalf("expected task URL in output, got:\n%s", buf.String())
	}
}

func TestRun_CreateTaskError_Propagates(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{
		createTaskErr: errors.New("asana 401"),
	}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	err := Run(&buf, Options{Message: "Fix"})
	if err == nil {
		t.Fatal("expected error from CreateTask")
	}
	if !strings.Contains(err.Error(), "asana 401") {
		t.Fatalf("expected wrapped error, got: %v", err)
	}
	// PR must NOT be created when task creation failed.
	if len(s.createPRCalls) != 0 {
		t.Fatalf("expected 0 PR calls, got %d", len(s.createPRCalls))
	}
}

// --- Verbosity modes ---

func TestRun_NormalMode_PassesWithSilentToGitCalls(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	if err := Run(&buf, Options{Message: "Fix bug"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.addCommitOptCount == 0 {
		t.Fatal("expected Normal mode to pass WithSilent to git calls (opt count > 0)")
	}
}

func TestRun_VerboseMode_PassesNoExecOpts(t *testing.T) {
	s := stubSubmitDeps(t)

	var buf bytes.Buffer
	if err := Run(&buf, Options{Message: "Fix bug", Verbose: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.addCommitOptCount != 0 {
		t.Fatalf("expected Verbose mode to pass no exec opts, got %d", s.addCommitOptCount)
	}
}

func TestRun_QuietMode_OutputIsOnlyURL(t *testing.T) {
	s := stubSubmitDeps(t)
	s.createPRURL = "https://github.com/owner/repo/pull/42"

	var buf bytes.Buffer
	if err := Run(&buf, Options{Message: "Fix bug", Quiet: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()
	// Must be exactly the URL on its own line (no prefix, no checkmark).
	if strings.TrimRight(output, "\n") != "https://github.com/owner/repo/pull/42" {
		t.Fatalf("expected Quiet mode to output URL only, got: %q", output)
	}
	// Also passes silent to git calls.
	if s.addCommitOptCount == 0 {
		t.Fatal("expected Quiet mode to pass WithSilent to git calls")
	}
}

func TestRun_NormalMode_OutputHasProgressAndURL(t *testing.T) {
	s := stubSubmitDeps(t)
	s.createPRURL = "https://github.com/owner/repo/pull/42"

	var buf bytes.Buffer
	if err := Run(&buf, Options{Message: "Fix bug"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()
	// Should contain checkmark progress lines.
	if !strings.Contains(output, "Committed") {
		t.Fatalf("expected Normal mode to contain 'Committed' progress line, got: %q", output)
	}
	if !strings.Contains(output, "Pushed") {
		t.Fatalf("expected Normal mode to contain 'Pushed' progress line, got: %q", output)
	}
	if !strings.Contains(output, "PR created") {
		t.Fatalf("expected Normal mode to contain 'PR created' line, got: %q", output)
	}
	// URL must be on its own line.
	if !strings.Contains(output, "https://github.com/owner/repo/pull/42") {
		t.Fatalf("expected Normal mode to contain PR URL, got: %q", output)
	}
}

func TestRun_VerboseMode_HasBranchAndTaskLines(t *testing.T) {
	s := stubSubmitDeps(t)
	tracker := &fakeTracker{createTaskReturn: tasktracker.Task{ID: "123", URL: "https://example/task/123"}}
	s.newTrackerReturn = tracker

	var buf bytes.Buffer
	if err := Run(&buf, Options{Message: "Fix bug", Verbose: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "PR created:") {
		t.Fatalf("expected Verbose mode to contain 'PR created:' line, got: %q", output)
	}
	if !strings.Contains(output, "Branch:") {
		t.Fatalf("expected Verbose mode to contain 'Branch:' line, got: %q", output)
	}
	if !strings.Contains(output, "Task:") {
		t.Fatalf("expected Verbose mode to contain 'Task:' line, got: %q", output)
	}
}

func TestRun_QuietMode_AutoMergeFailureNotOnStdout(t *testing.T) {
	s := stubSubmitDeps(t)
	s.autoMergeErr = errors.New("auto-merge not supported")
	s.createPRURL = "https://github.com/owner/repo/pull/99"

	// Capture the stubbed stderr so we can assert warnings went there.
	origStderr := stderr
	var errBuf bytes.Buffer
	stderr = &errBuf
	t.Cleanup(func() { stderr = origStderr })

	var buf bytes.Buffer
	if err := Run(&buf, Options{Message: "Fix", Quiet: true}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	stdoutStr := strings.TrimRight(buf.String(), "\n")
	if stdoutStr != "https://github.com/owner/repo/pull/99" {
		t.Fatalf("expected Quiet stdout to be URL only, got: %q", stdoutStr)
	}
	if !strings.Contains(errBuf.String(), "auto-merge") {
		t.Fatalf("expected auto-merge warning on stderr, got: %q", errBuf.String())
	}
}
