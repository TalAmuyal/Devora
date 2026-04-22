package cli

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"testing"
)

// --- Test harness ---

// captureStdout redirects os.Stdout to a pipe for the duration of the test.
// The returned closure restores the original Stdout and returns whatever
// was written during the test.
func captureStdout(t *testing.T) func() string {
	t.Helper()
	orig := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("os.Pipe: %v", err)
	}
	os.Stdout = w
	return func() string {
		w.Close()
		os.Stdout = orig
		var buf bytes.Buffer
		_, _ = io.Copy(&buf, r)
		return buf.String()
	}
}

// autoMergeStubs groups the package-level stubbable vars the test can
// override. All fields default to their production counterparts; tests mutate
// what they need.
type autoMergeStubs struct {
	repoSetValue      *bool
	repoSetErr        error
	repoUnsetCalled   bool
	repoUnsetErr      error
	repoRawReturn     *bool
	repoRawErr        error
	profileSetValue   **bool // pointer-to-pointer: first nil means "never called"
	profileSetErr     error
	profileRawReturn  *bool
	globalSetValue    **bool
	globalSetErr      error
	globalRawReturn   *bool
	resolveReturnName string
	resolveErr        error
}

// stubAutoMergeDeps replaces every stubbable var with a recorder/controller
// for the duration of the test. Returns the struct tests mutate.
func stubAutoMergeDeps(t *testing.T) *autoMergeStubs {
	t.Helper()
	s := &autoMergeStubs{}

	origSetGlobal := setPrAutoMergeGlobal
	origSetProfile := setPrAutoMergeProfile
	origGetGlobal := getPrAutoMergeGlobal
	origGetProfile := getPrAutoMergeProfile
	origSetRepo := setRepoAutoMerge
	origUnsetRepo := unsetRepoAutoMerge
	origGetRepoRaw := getRepoAutoMergeRaw

	setPrAutoMergeGlobal = func(v *bool) error {
		s.globalSetValue = &v
		return s.globalSetErr
	}
	setPrAutoMergeProfile = func(v *bool) error {
		s.profileSetValue = &v
		return s.profileSetErr
	}
	getPrAutoMergeGlobal = func() *bool {
		return s.globalRawReturn
	}
	getPrAutoMergeProfile = func() *bool {
		return s.profileRawReturn
	}
	setRepoAutoMerge = func(v bool) error {
		s.repoSetValue = &v
		return s.repoSetErr
	}
	unsetRepoAutoMerge = func() error {
		s.repoUnsetCalled = true
		return s.repoUnsetErr
	}
	getRepoAutoMergeRaw = func() (*bool, error) {
		return s.repoRawReturn, s.repoRawErr
	}

	stubResolveActiveProfile(t, func(string) (string, error) {
		return s.resolveReturnName, s.resolveErr
	})

	t.Cleanup(func() {
		setPrAutoMergeGlobal = origSetGlobal
		setPrAutoMergeProfile = origSetProfile
		getPrAutoMergeGlobal = origGetGlobal
		getPrAutoMergeProfile = origGetProfile
		setRepoAutoMerge = origSetRepo
		unsetRepoAutoMerge = origUnsetRepo
		getRepoAutoMergeRaw = origGetRepoRaw
	})

	return s
}

// chdirTemp cd's into a fresh temp directory so `git.EnsureInRepo` fails
// predictably. Restores the previous cwd on cleanup.
func chdirTemp(t *testing.T) {
	t.Helper()
	orig, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd: %v", err)
	}
	dir := t.TempDir()
	if err := os.Chdir(dir); err != nil {
		t.Fatalf("chdir: %v", err)
	}
	t.Cleanup(func() { _ = os.Chdir(orig) })
}

// chdirGitRepo initialises a bare git repo in a temp directory and cd's into
// it so `git.EnsureInRepo` succeeds.
func chdirGitRepo(t *testing.T) {
	t.Helper()
	dir := t.TempDir()
	// git init is the minimum to satisfy EnsureInRepo (git rev-parse --git-dir)
	if err := runCommand(dir, "git", "init", "-q", "-b", "main"); err != nil {
		t.Fatalf("git init: %v", err)
	}
	orig, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd: %v", err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatalf("chdir: %v", err)
	}
	t.Cleanup(func() { _ = os.Chdir(orig) })
}

// runCommand runs cmd in dir and returns an error when it exits non-zero.
// Only used for test setup.
func runCommand(dir, name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(),
		"GIT_AUTHOR_NAME=test", "GIT_AUTHOR_EMAIL=test@test.com",
		"GIT_COMMITTER_NAME=test", "GIT_COMMITTER_EMAIL=test@test.com",
	)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("%s %v failed: %v\n%s", name, args, err, string(out))
	}
	return nil
}

// --- Tests: flag parsing ---

func TestRun_PRAutoMerge_NoVerb_UsageError(t *testing.T) {
	stubAutoMergeDeps(t)
	err := runPRAutoMerge([]string{})
	var ue *UsageError
	if !errors.As(err, &ue) {
		t.Fatalf("expected *UsageError, got %T: %v", err, err)
	}
	if !strings.Contains(ue.Message, "requires a verb") {
		t.Fatalf("expected 'requires a verb' in usage message, got: %q", ue.Message)
	}
}

func TestRun_PRAutoMerge_UnknownVerb_UsageError(t *testing.T) {
	stubAutoMergeDeps(t)
	err := runPRAutoMerge([]string{"nuke"})
	var ue *UsageError
	if !errors.As(err, &ue) {
		t.Fatalf("expected *UsageError, got %T: %v", err, err)
	}
	if !strings.Contains(ue.Message, "unknown verb") {
		t.Fatalf("expected 'unknown verb' in message, got: %q", ue.Message)
	}
}

func TestRun_PRAutoMerge_Help_PrintsUsage(t *testing.T) {
	stubAutoMergeDeps(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"--help"})
	out := stop()
	if err != nil {
		t.Fatalf("expected nil error for --help, got: %v", err)
	}
	if !strings.Contains(out, "usage: debi pr auto-merge") {
		t.Fatalf("expected usage on stdout, got: %q", out)
	}
}

func TestRun_PRAutoMerge_InvalidScope_UsageError(t *testing.T) {
	stubAutoMergeDeps(t)
	err := runPRAutoMerge([]string{"enable", "--scope=bogus"})
	var ue *UsageError
	if !errors.As(err, &ue) {
		t.Fatalf("expected *UsageError, got %T: %v", err, err)
	}
	if !strings.Contains(ue.Message, "invalid scope") {
		t.Fatalf("expected 'invalid scope', got: %q", ue.Message)
	}
}

func TestRun_PRAutoMerge_JSONWithNonShowVerb_UsageError(t *testing.T) {
	stubAutoMergeDeps(t)
	chdirGitRepo(t)
	err := runPRAutoMerge([]string{"enable", "--json"})
	var ue *UsageError
	if !errors.As(err, &ue) {
		t.Fatalf("expected *UsageError, got %T: %v", err, err)
	}
	if !strings.Contains(ue.Message, "--json is only valid with the show verb") {
		t.Fatalf("expected rejection message, got: %q", ue.Message)
	}
}

func TestRun_PRAutoMerge_ScopeEqualsSyntax(t *testing.T) {
	s := stubAutoMergeDeps(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"enable", "--scope=global"})
	_ = stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.globalSetValue == nil || *s.globalSetValue == nil {
		t.Fatal("expected SetPrAutoMergeGlobal to be called with non-nil pointer")
	}
	if **s.globalSetValue != true {
		t.Fatalf("expected true passed to global setter, got %v", **s.globalSetValue)
	}
}

func TestRun_PRAutoMerge_ScopeSpaceSyntax(t *testing.T) {
	s := stubAutoMergeDeps(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"enable", "--scope", "global"})
	_ = stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.globalSetValue == nil || *s.globalSetValue == nil {
		t.Fatal("expected SetPrAutoMergeGlobal to be called")
	}
}

func TestRun_PRAutoMerge_DefaultScopeIsRepo(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"enable"})
	_ = stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.repoSetValue == nil {
		t.Fatal("expected setRepoAutoMerge to be called")
	}
}

// --- Tests: enable / disable ---

func TestRun_PRAutoMerge_Enable_Repo_CallsSetRepo(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"enable", "--scope=repo"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.repoSetValue == nil || *s.repoSetValue != true {
		t.Fatalf("expected setRepoAutoMerge(true), got %v", s.repoSetValue)
	}
	if !strings.Contains(out, "enabled for this clone") {
		t.Fatalf("expected 'enabled for this clone' in stdout, got: %q", out)
	}
	if !strings.Contains(out, "git config --local devora.pr.auto-merge=true") {
		t.Fatalf("expected storage key line, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Enable_Repo_NotInGitRepo(t *testing.T) {
	stubAutoMergeDeps(t)
	chdirTemp(t)
	err := runPRAutoMerge([]string{"enable", "--scope=repo"})
	var ue *UsageError
	if !errors.As(err, &ue) {
		t.Fatalf("expected *UsageError, got %T: %v", err, err)
	}
}

func TestRun_PRAutoMerge_Disable_Repo_CallsSetRepoFalse(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"disable", "--scope=repo"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.repoSetValue == nil || *s.repoSetValue != false {
		t.Fatalf("expected setRepoAutoMerge(false), got %v", s.repoSetValue)
	}
	if !strings.Contains(out, "disabled for this clone") {
		t.Fatalf("expected 'disabled for this clone' in stdout, got: %q", out)
	}
	if !strings.Contains(out, "devora.pr.auto-merge=false") {
		t.Fatalf("expected 'devora.pr.auto-merge=false' in stdout, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Enable_Profile_CallsProfileSetter(t *testing.T) {
	s := stubAutoMergeDeps(t)
	s.resolveReturnName = "work"
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"enable", "--scope=profile"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.profileSetValue == nil || *s.profileSetValue == nil {
		t.Fatal("expected profile setter called with non-nil pointer")
	}
	if **s.profileSetValue != true {
		t.Fatalf("expected true, got %v", **s.profileSetValue)
	}
	if !strings.Contains(out, `profile "work"`) {
		t.Fatalf("expected output to mention profile name, got: %q", out)
	}
	if !strings.Contains(out, "config.json: pr.auto-merge=true") {
		t.Fatalf("expected storage key line, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Enable_Profile_NoActiveProfile(t *testing.T) {
	stubAutoMergeDeps(t)
	// resolveReturnName stays "" and err stays nil -> simulates "no active profile".
	err := runPRAutoMerge([]string{"enable", "--scope=profile"})
	var ue *UsageError
	if !errors.As(err, &ue) {
		t.Fatalf("expected *UsageError, got %T: %v", err, err)
	}
	if !strings.Contains(ue.Message, "no active profile") {
		t.Fatalf("expected 'no active profile' in message, got: %q", ue.Message)
	}
}

func TestRun_PRAutoMerge_Enable_Global_CallsGlobalSetter(t *testing.T) {
	s := stubAutoMergeDeps(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"enable", "--scope=global"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.globalSetValue == nil || *s.globalSetValue == nil {
		t.Fatal("expected global setter called with non-nil pointer")
	}
	if **s.globalSetValue != true {
		t.Fatalf("expected true, got %v", **s.globalSetValue)
	}
	if !strings.Contains(out, "enabled globally") {
		t.Fatalf("expected 'enabled globally' in output, got: %q", out)
	}
	if !strings.Contains(out, "~/.config/devora/config.json: pr.auto-merge=true") {
		t.Fatalf("expected storage key line, got: %q", out)
	}
}

// --- Tests: reset ---

func TestRun_PRAutoMerge_Reset_Repo_Cleared(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	existing := true
	s.repoRawReturn = &existing
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"reset"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !s.repoUnsetCalled {
		t.Fatal("expected unsetRepoAutoMerge to be called")
	}
	if !strings.Contains(out, "cleared per-repo auto-merge") {
		t.Fatalf("expected 'cleared' message, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Reset_Repo_AlreadyUnset(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	// repoRawReturn stays nil
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"reset"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !s.repoUnsetCalled {
		t.Fatal("expected unset call even when already unset (idempotent)")
	}
	if !strings.Contains(out, "already unset for this clone") {
		t.Fatalf("expected 'already unset' message, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Reset_Profile_Cleared(t *testing.T) {
	s := stubAutoMergeDeps(t)
	s.resolveReturnName = "work"
	existing := false
	s.profileRawReturn = &existing
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"reset", "--scope=profile"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.profileSetValue == nil {
		t.Fatal("expected profile setter to be called (reset -> nil)")
	}
	if *s.profileSetValue != nil {
		t.Fatalf("expected profile setter called with nil pointer, got %v", **s.profileSetValue)
	}
	if !strings.Contains(out, `cleared auto-merge for profile "work"`) {
		t.Fatalf("expected 'cleared' message, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Reset_Profile_AlreadyUnset(t *testing.T) {
	s := stubAutoMergeDeps(t)
	s.resolveReturnName = "work"
	// profileRawReturn nil
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"reset", "--scope=profile"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.profileSetValue == nil {
		t.Fatal("expected profile setter to be called even when unset")
	}
	if !strings.Contains(out, `already unset for profile "work"`) {
		t.Fatalf("expected 'already unset' message, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Reset_Global_Cleared(t *testing.T) {
	s := stubAutoMergeDeps(t)
	existing := true
	s.globalRawReturn = &existing
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"reset", "--scope=global"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.globalSetValue == nil {
		t.Fatal("expected global setter to be called")
	}
	if *s.globalSetValue != nil {
		t.Fatalf("expected global setter called with nil pointer, got %v", **s.globalSetValue)
	}
	if !strings.Contains(out, "cleared global auto-merge") {
		t.Fatalf("expected 'cleared' message, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Reset_Global_AlreadyUnset(t *testing.T) {
	s := stubAutoMergeDeps(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"reset", "--scope=global"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if s.globalSetValue == nil {
		t.Fatal("expected global setter to be called even when unset")
	}
	if !strings.Contains(out, "already unset globally") {
		t.Fatalf("expected 'already unset' message, got: %q", out)
	}
}

// --- Tests: show ---

func TestRun_PRAutoMerge_Show_Human_AllLayersUnset(t *testing.T) {
	stubAutoMergeDeps(t)
	chdirTemp(t)
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"show"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(out, "effective: true  (from: default)") {
		t.Fatalf("expected 'effective: true  (from: default)' in output, got: %q", out)
	}
	if !strings.Contains(out, "repo:      <unset>") {
		t.Fatalf("expected repo <unset>, got: %q", out)
	}
	if !strings.Contains(out, "profile:   <unset>") {
		t.Fatalf("expected profile <unset>, got: %q", out)
	}
	if !strings.Contains(out, "global:    <unset>") {
		t.Fatalf("expected global <unset>, got: %q", out)
	}
	if !strings.Contains(out, "default:   true") {
		t.Fatalf("expected default line, got: %q", out)
	}
}

func TestRun_PRAutoMerge_Show_Human_RepoWins(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	repoVal := false
	s.repoRawReturn = &repoVal
	profVal := true
	s.profileRawReturn = &profVal
	globVal := true
	s.globalRawReturn = &globVal
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"show"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(out, "effective: false  (from: repo)") {
		t.Fatalf("expected 'effective: false  (from: repo)', got: %q", out)
	}
	if !strings.Contains(out, "repo:      false") {
		t.Fatalf("expected 'repo:      false', got: %q", out)
	}
	if !strings.Contains(out, "profile:   true") {
		t.Fatalf("expected 'profile:   true', got: %q", out)
	}
	if !strings.Contains(out, "global:    true") {
		t.Fatalf("expected 'global:    true', got: %q", out)
	}
}

func TestRun_PRAutoMerge_Show_Human_ProfileWinsWhenRepoUnset(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	profVal := false
	s.profileRawReturn = &profVal
	globVal := true
	s.globalRawReturn = &globVal
	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"show"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(out, "effective: false  (from: profile)") {
		t.Fatalf("expected 'from: profile', got: %q", out)
	}
}

func TestRun_PRAutoMerge_Show_JSON_EmitsSchema(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirGitRepo(t)
	repoVal := true
	s.repoRawReturn = &repoVal
	globVal := false
	s.globalRawReturn = &globVal

	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"show", "--json"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Must be single line (encoder appends a single newline).
	lines := strings.Split(strings.TrimRight(out, "\n"), "\n")
	if len(lines) != 1 {
		t.Fatalf("expected single-line JSON, got %d lines: %q", len(lines), out)
	}
	var payload struct {
		Key       string `json:"key"`
		Effective bool   `json:"effective"`
		Source    string `json:"source"`
		Layers    struct {
			Repo    *bool `json:"repo"`
			Profile *bool `json:"profile"`
			Global  *bool `json:"global"`
			Default bool  `json:"default"`
		} `json:"layers"`
	}
	if err := json.Unmarshal([]byte(lines[0]), &payload); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, out)
	}
	if payload.Key != "pr.auto-merge" {
		t.Fatalf("expected key 'pr.auto-merge', got %q", payload.Key)
	}
	if payload.Effective != true {
		t.Fatalf("expected effective=true, got %v", payload.Effective)
	}
	if payload.Source != "repo" {
		t.Fatalf("expected source='repo', got %q", payload.Source)
	}
	if payload.Layers.Repo == nil || *payload.Layers.Repo != true {
		t.Fatalf("expected layers.repo=true, got %v", payload.Layers.Repo)
	}
	if payload.Layers.Profile != nil {
		t.Fatalf("expected layers.profile=null, got %v", *payload.Layers.Profile)
	}
	if payload.Layers.Global == nil || *payload.Layers.Global != false {
		t.Fatalf("expected layers.global=false, got %v", payload.Layers.Global)
	}
	if payload.Layers.Default != true {
		t.Fatalf("expected layers.default=true, got %v", payload.Layers.Default)
	}
}

func TestRun_PRAutoMerge_Show_OutsideGitRepo_SkipsRepoLayer(t *testing.T) {
	s := stubAutoMergeDeps(t)
	chdirTemp(t)
	globVal := false
	s.globalRawReturn = &globVal

	stop := captureStdout(t)
	err := runPRAutoMerge([]string{"show"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Repo layer silently unset; global wins.
	if !strings.Contains(out, "repo:      <unset>") {
		t.Fatalf("expected repo <unset> outside git repo, got: %q", out)
	}
	if !strings.Contains(out, "effective: false  (from: global)") {
		t.Fatalf("expected 'from: global', got: %q", out)
	}
}
