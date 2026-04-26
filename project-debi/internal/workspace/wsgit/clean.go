package wsgit

import (
	"fmt"
	"io"
	"path/filepath"
	"sort"
	"sync"

	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/workspace"
)

// RunClean implements workspace-mode `debi gcl`: a two-phase verify-then-update
// flow. Phase 1 verifies every repo is clean and on detached HEAD; if any
// repo fails, we abort and return a non-zero PassthroughError so no fetches
// happen. Phase 2 runs `git fetch origin` and `git checkout origin/<default>`
// in parallel and reports per-repo success.
func RunClean(w io.Writer, wsPath string) error {
	repos, err := workspace.GetWorkspaceRepos(wsPath)
	if err != nil {
		return fmt.Errorf("list workspace repos: %w", err)
	}

	wsName := filepath.Base(wsPath)
	fmt.Fprintf(w, "WORKSPACE  %s (%d repos)\n\n", wsName, len(repos))

	verifyResults := runVerifyPhase(wsPath, repos)
	sort.Slice(verifyResults, func(i, j int) bool {
		return verifyResults[i].Name < verifyResults[j].Name
	})

	if hasVerifyFailures(verifyResults) {
		renderVerifyFailure(w, verifyResults)
		return &process.PassthroughError{Code: 1}
	}

	cleanResults := runCleanPhase(w, wsPath, repos)
	sort.Slice(cleanResults, func(i, j int) bool {
		return cleanResults[i].Name < cleanResults[j].Name
	})

	renderCleanResults(w, cleanResults)
	if hasCleanFailures(cleanResults) {
		return &process.PassthroughError{Code: 1}
	}
	return nil
}

func runVerifyPhase(wsPath string, repos []string) []RepoVerifyResult {
	results := make([]RepoVerifyResult, len(repos))
	var wg sync.WaitGroup
	for i, repo := range repos {
		wg.Add(1)
		go func(idx int, name string) {
			defer wg.Done()
			results[idx] = verifyRepo(filepath.Join(wsPath, name), name)
		}(i, repo)
	}
	wg.Wait()
	return results
}

// verifyRepo checks a single repo's eligibility for the gcl flow: clean
// working tree and detached HEAD.
func verifyRepo(repoPath, name string) RepoVerifyResult {
	res := RepoVerifyResult{Name: name}

	clean, err := workspace.IsRepoClean(repoPath)
	if err != nil {
		res.Err = err
		return res
	}
	res.Clean = clean

	branch, err := git.CurrentBranchOrDetached(process.WithCwd(repoPath))
	if err != nil {
		res.Err = err
		return res
	}
	res.Branch = branch
	res.Detached = branch == ""
	return res
}

func hasVerifyFailures(results []RepoVerifyResult) bool {
	for _, r := range results {
		if r.Err != nil || !r.Clean || !r.Detached {
			return true
		}
	}
	return false
}

func renderVerifyFailure(w io.Writer, results []RepoVerifyResult) {
	fmt.Fprintln(w, "Phase 1: verify all repos clean and detached…")

	nameWidth := 0
	for _, r := range results {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
	}

	failed := 0
	for _, r := range results {
		ok, reason := verifyRowSummary(r)
		if ok {
			fmt.Fprintf(w, "  %s  %s\n", greenStyle.Render("✓"), r.Name)
			continue
		}
		failed++
		fmt.Fprintf(w, "  %s  %-*s  %s\n", redStyle.Render("✗"), nameWidth, r.Name, reason)
	}

	fmt.Fprintln(w)
	fmt.Fprintf(w, "%s  Aborted — %d of %d repos failed verification. No changes made.\n",
		redStyle.Render("✗"), failed, len(results))
	fmt.Fprintln(w, "   Fix the listed repos and re-run `debi gcl`.")
}

// verifyRowSummary returns (ok, reason). When ok is false, reason is a short
// human-readable explanation: "verify error: ...", "dirty", or "on branch X".
func verifyRowSummary(r RepoVerifyResult) (bool, string) {
	if r.Err != nil {
		return false, "verify error: " + r.Err.Error()
	}
	if !r.Clean {
		return false, "dirty"
	}
	if !r.Detached {
		return false, "on branch " + r.Branch
	}
	return true, ""
}

// cleanRepoResult is the per-repo outcome of Phase 2.
type cleanRepoResult struct {
	Name    string
	Branch  string // origin/<default> on success; empty on failure
	Err     error
}

func runCleanPhase(w io.Writer, wsPath string, repos []string) []cleanRepoResult {
	fmt.Fprintln(w, "Phase 2: fetch + checkout origin/<default> in parallel…")

	results := make([]cleanRepoResult, len(repos))
	var wg sync.WaitGroup
	for i, repo := range repos {
		wg.Add(1)
		go func(idx int, name string) {
			defer wg.Done()
			results[idx] = runGclInRepo(filepath.Join(wsPath, name), name)
		}(i, repo)
	}
	wg.Wait()
	return results
}

// runGclInRepo composes `git fetch origin` + `git checkout origin/<default>`
// using captured output. Composing here (not calling git.Gcl) keeps
// internal/git purely passthrough; capturing avoids interleaved progress
// output across N parallel repos.
func runGclInRepo(repoPath, name string) cleanRepoResult {
	res := cleanRepoResult{Name: name}

	if err := bestEffortFetch(repoPath); err != nil {
		res.Err = fmt.Errorf("fetch: %w", err)
		return res
	}

	mainBranch, err := git.DefaultBranchNameWithFallback(process.WithCwd(repoPath))
	if err != nil {
		res.Err = fmt.Errorf("resolve default branch: %w", err)
		return res
	}

	target := "origin/" + mainBranch
	if _, err := process.GetOutput(
		[]string{"git", "checkout", target},
		process.WithCwd(repoPath),
	); err != nil {
		res.Err = fmt.Errorf("checkout %s: %w", target, err)
		return res
	}
	res.Branch = target
	return res
}

func hasCleanFailures(results []cleanRepoResult) bool {
	for _, r := range results {
		if r.Err != nil {
			return true
		}
	}
	return false
}

func renderCleanResults(w io.Writer, results []cleanRepoResult) {
	nameWidth := 0
	for _, r := range results {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
	}

	succeeded := 0
	for _, r := range results {
		if r.Err == nil {
			succeeded++
			fmt.Fprintf(w, "  %s  %-*s  checked out %s\n",
				greenStyle.Render("✓"), nameWidth, r.Name, r.Branch)
			continue
		}
		fmt.Fprintf(w, "  %s  %-*s  %s\n",
			redStyle.Render("✗"), nameWidth, r.Name, r.Err.Error())
	}

	fmt.Fprintln(w)
	if succeeded == len(results) {
		fmt.Fprintf(w, "%s  Done — %d of %d repos updated.\n",
			greenStyle.Render("✓"), succeeded, len(results))
		return
	}
	fmt.Fprintf(w, "%s  Done — %d of %d repos updated; %d failed.\n",
		redStyle.Render("✗"), succeeded, len(results), len(results)-succeeded)
}
