package wsgit

import (
	"bytes"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"

	"devora/internal/config"
	"devora/internal/gh"
	"devora/internal/process"
)

// --- Test helpers (mirror internal/workspace/worktree_test.go) ---

func runGit(t *testing.T, cwd string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = cwd
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v failed in %s: %v\nOutput: %s", args, cwd, err, string(out))
	}
	return strings.TrimSpace(string(out))
}

func initGitRepo(t *testing.T, repoPath string) {
	t.Helper()
	if err := os.MkdirAll(repoPath, 0o755); err != nil {
		t.Fatalf("failed to create repo dir: %v", err)
	}
	runGit(t, repoPath, "init", "-b", "main")
	runGit(t, repoPath, "config", "user.email", "test@test.com")
	runGit(t, repoPath, "config", "user.name", "Test")
	dummyFile := filepath.Join(repoPath, "README.md")
	if err := os.WriteFile(dummyFile, []byte("# Test Repo\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	runGit(t, repoPath, "add", ".")
	runGit(t, repoPath, "commit", "-m", "initial commit")
}

// detachHead leaves the repo on detached HEAD at the current commit.
func detachHead(t *testing.T, repoPath string) {
	t.Helper()
	commit := runGit(t, repoPath, "rev-parse", "HEAD")
	runGit(t, repoPath, "checkout", "--detach", commit)
}

// --- Stub helpers ---

func stubGetPRForBranch(t *testing.T, fn func(branch string, opts ...process.ExecOption) (*gh.PRSummary, error)) {
	t.Helper()
	orig := getPRForBranch
	getPRForBranch = fn
	t.Cleanup(func() { getPRForBranch = orig })
}

// --- verifyRepo ---

func TestVerifyRepo_CleanAndDetached(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	detachHead(t, repo)

	res := verifyRepo(repo, "repo")
	if res.Err != nil {
		t.Fatalf("unexpected error: %v", res.Err)
	}
	if !res.Clean {
		t.Fatal("expected Clean=true")
	}
	if !res.Detached {
		t.Fatalf("expected Detached=true, got branch=%q", res.Branch)
	}
}

func TestVerifyRepo_Dirty(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	detachHead(t, repo)

	if err := os.WriteFile(filepath.Join(repo, "dirty.txt"), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}

	res := verifyRepo(repo, "repo")
	if res.Err != nil {
		t.Fatalf("unexpected error: %v", res.Err)
	}
	if res.Clean {
		t.Fatal("expected Clean=false for dirty repo")
	}
}

func TestVerifyRepo_OnBranch(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)

	res := verifyRepo(repo, "repo")
	if res.Err != nil {
		t.Fatalf("unexpected error: %v", res.Err)
	}
	if res.Detached {
		t.Fatal("expected Detached=false for on-branch repo")
	}
	if res.Branch == "" {
		t.Fatal("expected branch name to be populated")
	}
}

// --- EnsureAtWorkspaceRoot ---

// withWorkspacesRoot points the global config at a fresh temp dir and registers
// a profile whose root path is profileRoot. workspace.ResolveWorkspaceFromCWD
// then treats <profileRoot>/workspaces as the workspaces root.
func withWorkspacesRoot(t *testing.T, profileRoot string) {
	t.Helper()

	configRoot := t.TempDir()
	configFile := filepath.Join(configRoot, "config.json")

	if err := os.MkdirAll(profileRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	profileCfg := `{"name":"test"}`
	if err := os.WriteFile(filepath.Join(profileRoot, "config.json"), []byte(profileCfg), 0o644); err != nil {
		t.Fatal(err)
	}

	globalCfg := `{"profiles":["` + profileRoot + `"]}`
	if err := os.WriteFile(configFile, []byte(globalCfg), 0o644); err != nil {
		t.Fatal(err)
	}

	origPath := config.ConfigPath()
	config.SetConfigPathForTesting(configFile)
	t.Cleanup(func() { config.SetConfigPathForTesting(origPath) })
}

func makeWorkspace(t *testing.T, root string, name string, repoNames ...string) string {
	t.Helper()
	wsRoot := filepath.Join(root, "workspaces")
	wsPath := filepath.Join(wsRoot, name)
	if err := os.MkdirAll(wsPath, 0o755); err != nil {
		t.Fatal(err)
	}
	// Initialized marker so workspace.ResolveWorkspaceFromCWD recognizes it.
	if err := os.WriteFile(filepath.Join(wsPath, "initialized"), []byte{}, 0o644); err != nil {
		t.Fatal(err)
	}
	for _, repoName := range repoNames {
		repoPath := filepath.Join(wsPath, repoName)
		initGitRepo(t, repoPath)
	}
	return wsPath
}

func TestEnsureAtWorkspaceRoot_AtRoot_Success(t *testing.T) {
	rootDir := t.TempDir()
	withWorkspacesRoot(t, rootDir)
	wsPath := makeWorkspace(t, rootDir, "ws-1")

	got, err := EnsureAtWorkspaceRoot(wsPath)
	if err != nil {
		t.Fatalf("expected success, got err: %v", err)
	}
	resolved, _ := filepath.EvalSymlinks(wsPath)
	if got != resolved {
		t.Fatalf("expected wsPath %q, got %q", resolved, got)
	}
}

func TestEnsureAtWorkspaceRoot_InsideRepo_NotAtRoot(t *testing.T) {
	rootDir := t.TempDir()
	withWorkspacesRoot(t, rootDir)
	wsPath := makeWorkspace(t, rootDir, "ws-1", "repo-a")

	insideRepo := filepath.Join(wsPath, "repo-a")
	_, err := EnsureAtWorkspaceRoot(insideRepo)
	if !errors.Is(err, ErrNotAtWorkspaceRoot) {
		t.Fatalf("expected ErrNotAtWorkspaceRoot, got: %v", err)
	}
}

func TestEnsureAtWorkspaceRoot_OutsideAnyWorkspace(t *testing.T) {
	rootDir := t.TempDir()
	withWorkspacesRoot(t, rootDir)
	makeWorkspace(t, rootDir, "ws-1")

	outside := t.TempDir()
	_, err := EnsureAtWorkspaceRoot(outside)
	if !errors.Is(err, ErrNotAtWorkspaceRoot) {
		t.Fatalf("expected ErrNotAtWorkspaceRoot, got: %v", err)
	}
}

// --- gatherRepoStatus ---

func TestGatherRepoStatus_CleanDetachedNoRemote(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	detachHead(t, repo)

	stubGetPRForBranch(t, func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		t.Fatal("getPRForBranch should not be called when detached")
		return nil, nil
	})

	var ghMissing atomic.Bool
	res := gatherRepoStatus(repo, "repo", &ghMissing)

	if res.Err != nil {
		t.Fatalf("unexpected err: %v", res.Err)
	}
	if res.Branch != "" {
		t.Fatalf("expected detached (Branch=\"\"), got %q", res.Branch)
	}
	if res.Counts.Staged != 0 || res.Counts.Unstaged != 0 || res.Counts.Untracked != 0 {
		t.Fatalf("expected zero counts, got %+v", res.Counts)
	}
	if res.PRState != PRStateNone {
		t.Fatalf("expected PR=PRStateNone, got %q", res.PRState)
	}
}

func TestGatherRepoStatus_DirtyAndUntracked(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)

	if err := os.WriteFile(filepath.Join(repo, "README.md"), []byte("# changed\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(repo, "new.txt"), []byte("new\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	stubGetPRForBranch(t, func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return nil, nil
	})

	var ghMissing atomic.Bool
	res := gatherRepoStatus(repo, "repo", &ghMissing)

	if res.Counts.Unstaged != 1 {
		t.Fatalf("expected 1 unstaged, got %d", res.Counts.Unstaged)
	}
	if res.Counts.Untracked != 1 {
		t.Fatalf("expected 1 untracked, got %d", res.Counts.Untracked)
	}
}

func TestGatherRepoStatus_FetchFailure_StillRenders(t *testing.T) {
	// Fresh init has no 'origin' remote, so `git fetch origin` will fail.
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	detachHead(t, repo)

	stubGetPRForBranch(t, func(string, ...process.ExecOption) (*gh.PRSummary, error) { return nil, nil })

	var ghMissing atomic.Bool
	res := gatherRepoStatus(repo, "repo", &ghMissing)

	if !res.FetchFailed {
		t.Fatal("expected FetchFailed=true when no origin remote")
	}
	if res.Err != nil {
		t.Fatalf("fetch failure should not propagate as Err: %v", res.Err)
	}
}

// TestGatherRepoStatus_PassesCwdToGH guards against the regression where
// gh.GetPRForBranch inherited the workspace-root cwd instead of the repo's
// cwd. The fix threads process.WithCwd(repoPath) into the gh subprocess.
// We assert it indirectly: apply the opts received by getPRForBranch to a
// real `pwd` subprocess and check the resolved directory matches repoPath.
func TestGatherRepoStatus_PassesCwdToGH(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	// Stay on branch so PR lookup actually triggers.

	var capturedCwd string
	stubGetPRForBranch(t, func(branch string, opts ...process.ExecOption) (*gh.PRSummary, error) {
		// Apply opts to a pwd invocation; the subprocess's working
		// directory is the most direct evidence of WithCwd taking effect.
		out, err := process.GetOutput([]string{"pwd"}, opts...)
		if err != nil {
			t.Fatalf("pwd via forwarded opts failed: %v", err)
		}
		capturedCwd = out
		return nil, nil
	})

	var ghMissing atomic.Bool
	gatherRepoStatus(repo, "repo", &ghMissing)

	expected, _ := filepath.EvalSymlinks(repo)
	got, _ := filepath.EvalSymlinks(capturedCwd)
	if got != expected {
		t.Fatalf("expected gh subprocess cwd %q, got %q", expected, got)
	}
}

func TestGatherRepoStatus_GhMissing_SkipsLookup(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	// Stay on branch so PR lookup would normally trigger.

	calls := 0
	stubGetPRForBranch(t, func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		calls++
		return nil, gh.ErrGHNotInstalled
	})

	var ghMissing atomic.Bool
	res := gatherRepoStatus(repo, "repo", &ghMissing)

	if !ghMissing.Load() {
		t.Fatal("expected ghMissing flag to be set after first call")
	}
	if calls != 1 {
		t.Fatalf("expected 1 call, got %d", calls)
	}
	if res.PRState != PRStateUnknown {
		t.Fatalf("expected PRStateUnknown when gh missing, got %q", res.PRState)
	}
}

// --- RunStatus ---

func TestRunStatus_AlphabeticalAndOneBadRepoDoesntSilenceOthers(t *testing.T) {
	wsPath := t.TempDir()
	repoA := filepath.Join(wsPath, "alpha")
	repoB := filepath.Join(wsPath, "beta")
	initGitRepo(t, repoA)
	initGitRepo(t, repoB)
	// Detach beta, leave alpha on a branch.
	detachHead(t, repoB)

	stubGetPRForBranch(t, func(string, ...process.ExecOption) (*gh.PRSummary, error) {
		return nil, nil
	})

	var buf bytes.Buffer
	if err := RunStatus(&buf, wsPath); err != nil {
		t.Fatalf("RunStatus error: %v", err)
	}
	out := buf.String()
	idxA := strings.Index(out, "alpha")
	idxB := strings.Index(out, "beta")
	if idxA < 0 || idxB < 0 {
		t.Fatalf("expected both repo names in output, got:\n%s", out)
	}
	if !(idxA < idxB) {
		t.Fatalf("expected alpha before beta in output, got:\n%s", out)
	}
	if !strings.Contains(out, "WORKSPACE") || !strings.Contains(out, "SUMMARY") {
		t.Fatalf("expected header and summary in output, got:\n%s", out)
	}
}

// --- RunClean Phase 1 abort ---

func TestRunClean_Phase1Abort_DirtyRepo(t *testing.T) {
	wsPath := t.TempDir()
	repoA := filepath.Join(wsPath, "clean-repo")
	repoB := filepath.Join(wsPath, "dirty-repo")
	initGitRepo(t, repoA)
	initGitRepo(t, repoB)
	detachHead(t, repoA)
	detachHead(t, repoB)

	if err := os.WriteFile(filepath.Join(repoB, "dirty.txt"), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}

	var buf bytes.Buffer
	err := RunClean(&buf, wsPath)
	if err == nil {
		t.Fatal("expected error when a repo fails verification")
	}
	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) || ptErr.Code != 1 {
		t.Fatalf("expected PassthroughError{Code:1}, got %T: %v", err, err)
	}

	out := buf.String()
	if !strings.Contains(out, "dirty-repo") {
		t.Fatalf("expected dirty repo named in output:\n%s", out)
	}
	if !strings.Contains(out, "Aborted") {
		t.Fatalf("expected abort summary in output:\n%s", out)
	}
	// No fetch should happen — verify that the FETCH_HEAD file is absent.
	if _, statErr := os.Stat(filepath.Join(repoA, ".git", "FETCH_HEAD")); statErr == nil {
		t.Fatalf("expected no fetch on Phase 1 failure, but FETCH_HEAD exists in clean-repo")
	}
}

// --- RunClean Phase 2 happy path ---

// makeWorkspaceRepoWithOrigin creates origin.git outside wsPath, seeds it,
// then clones into wsPath/<name> in detached HEAD on origin/main. Returns the
// repo workdir.
func makeWorkspaceRepoWithOrigin(t *testing.T, originRoot, wsPath, name string) string {
	t.Helper()
	originDir := filepath.Join(originRoot, name+".git")
	if err := os.MkdirAll(originDir, 0o755); err != nil {
		t.Fatal(err)
	}
	runGit(t, originRoot, "init", "--bare", "-b", "main", filepath.Base(originDir))

	seed := filepath.Join(originRoot, name+"-seed")
	initGitRepo(t, seed)
	runGit(t, seed, "remote", "add", "origin", originDir)
	runGit(t, seed, "push", "origin", "main")

	repoPath := filepath.Join(wsPath, name)
	runGit(t, wsPath, "clone", originDir, name)
	runGit(t, repoPath, "checkout", "--detach", "origin/main")
	return repoPath
}

func TestRunClean_Phase2HappyPath(t *testing.T) {
	root := t.TempDir()
	originRoot := filepath.Join(root, "origins")
	if err := os.MkdirAll(originRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	wsPath := filepath.Join(root, "ws")
	if err := os.MkdirAll(wsPath, 0o755); err != nil {
		t.Fatal(err)
	}
	repoA := makeWorkspaceRepoWithOrigin(t, originRoot, wsPath, "alpha")
	repoB := makeWorkspaceRepoWithOrigin(t, originRoot, wsPath, "beta")

	var buf bytes.Buffer
	if err := RunClean(&buf, wsPath); err != nil {
		t.Fatalf("RunClean error: %v\noutput:\n%s", err, buf.String())
	}

	out := buf.String()
	if !strings.Contains(out, "alpha") || !strings.Contains(out, "beta") {
		t.Fatalf("expected both repos in output:\n%s", out)
	}
	if !strings.Contains(out, "Done") {
		t.Fatalf("expected 'Done' line:\n%s", out)
	}

	for _, repo := range []string{repoA, repoB} {
		head := runGit(t, repo, "rev-parse", "HEAD")
		expected := runGit(t, repo, "rev-parse", "origin/main")
		if head != expected {
			t.Fatalf("repo %s: HEAD=%s, expected origin/main=%s", repo, head, expected)
		}
	}
}
