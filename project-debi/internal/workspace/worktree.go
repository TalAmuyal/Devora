package workspace

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"devora/internal/config"
	"devora/internal/process"
)

func getRepoPath(repoName string) (string, error) {
	repos, err := config.GetRegisteredRepos()
	if err != nil {
		return "", err
	}
	for _, repoPath := range repos {
		if filepath.Base(repoPath) == repoName {
			return repoPath, nil
		}
	}
	return "", fmt.Errorf("unknown repo name: %s", repoName)
}

func getDefaultBranchName(repoPath string) (string, error) {
	output, err := process.GetOutput(
		[]string{"git", "symbolic-ref", "refs/remotes/origin/HEAD"},
		process.WithCwd(repoPath),
	)
	if err != nil {
		return "", err
	}
	return strings.TrimPrefix(output, "refs/remotes/origin/"), nil
}

func MakeAndPrepareWorkTree(workspacePath string, repoName string, worktreeDirName string) error {
	repoPath, err := getRepoPath(repoName)
	if err != nil {
		return err
	}

	branch, err := getDefaultBranchName(repoPath)
	if err != nil {
		return err
	}

	// Fetch
	_, err = process.GetOutput(
		[]string{"git", "fetch", "origin", branch},
		process.WithCwd(repoPath),
	)
	if err != nil {
		return err
	}

	// Create worktree
	targetPath := filepath.Join(workspacePath, worktreeDirName)
	if err := os.MkdirAll(filepath.Dir(targetPath), 0o755); err != nil {
		return err
	}
	_, err = process.GetOutput(
		[]string{"git", "worktree", "add", "--detach", targetPath, "origin/" + branch},
		process.WithCwd(repoPath),
	)
	if err != nil {
		return err
	}

	// Trust mise
	_, err = process.GetOutput(
		[]string{"mise", "trust"},
		process.WithCwd(targetPath),
	)
	if err != nil {
		return err
	}

	// Prepare
	prepareCommand := config.GetPrepareCommand()
	if prepareCommand != nil {
		_, err = process.GetShellOutput(*prepareCommand, process.WithCwd(targetPath))
		if err != nil {
			return err
		}
	}

	return nil
}

func GetRepoBranch(repoPath string) (string, error) {
	output, err := process.GetOutput(
		[]string{"git", "rev-parse", "--abbrev-ref", "HEAD"},
		process.WithCwd(repoPath),
	)
	if err != nil {
		return "", err
	}
	return output, nil
}

func IsRepoClean(repoPath string) (bool, error) {
	output, err := process.GetOutput(
		[]string{"git", "status", "--porcelain"},
		process.WithCwd(repoPath),
	)
	if err != nil {
		return false, err
	}
	return output == "", nil
}

func IsGitRepo(path string) bool {
	_, err := process.GetOutput(
		[]string{"git", "rev-parse", "--git-dir"},
		process.WithCwd(path),
	)
	return err == nil
}
