// Package workspace reads workspaces on disk: directories under a profile's workspaces root, each holding one git worktree per repo.
// The Devora app owns creating, activating, and deleting workspaces; debi only reads them, to resolve the profile from the CWD and to run the multi-repo git commands in wsgit.
//
// A workspace without the initialized marker is Invalid (its initialization failed or hasn't finished), so debi ignores it.
// The app also tells Active from Inactive workspaces by a task.json file, which debi doesn't need.
package workspace

import (
	"os"
	"path/filepath"
	"strings"

	"devora/internal/config"
)

const InitializedMarkerName = "initialized"

// --- Path Helpers ---

func getInitializationMarkerPath(workspacePath string) string {
	return filepath.Join(workspacePath, InitializedMarkerName)
}

// --- State Query Functions ---

func IsInitialized(workspacePath string) bool {
	_, err := os.Stat(getInitializationMarkerPath(workspacePath))
	return err == nil
}

// --- Workspace Enumeration ---

// GetWorkspaceRepos returns the names of the workspace's repos, in no guaranteed order.
// Dot-prefixed directories (e.g., .claude) are not repos.
func GetWorkspaceRepos(workspacePath string) ([]string, error) {
	entries, err := os.ReadDir(workspacePath)
	if err != nil {
		return nil, err
	}

	var repos []string
	for _, entry := range entries {
		if entry.IsDir() && !strings.HasPrefix(entry.Name(), ".") {
			repos = append(repos, entry.Name())
		}
	}
	return repos, nil
}

// --- Workspace Detection from CWD ---

// ResolveWorkspaceFromCWD returns the profile and workspace path containing cwd.
// Being outside any workspace is not an error: it returns (nil, "", nil).
func ResolveWorkspaceFromCWD(cwd string) (*config.Profile, string, error) {
	resolvedCWD, err := filepath.Abs(cwd)
	if err != nil {
		return nil, "", nil
	}
	resolvedCWD, err = filepath.EvalSymlinks(resolvedCWD)
	if err != nil {
		return nil, "", nil
	}

	profiles := config.GetProfiles()
	for i := range profiles {
		profile := &profiles[i]
		workspacesRoot := config.WorkspacesRootForProfile(profile)
		absRoot, err := filepath.Abs(workspacesRoot)
		if err != nil {
			continue
		}
		absRoot, err = filepath.EvalSymlinks(absRoot)
		if err != nil {
			continue
		}

		rel, err := filepath.Rel(absRoot, resolvedCWD)
		if err != nil {
			continue
		}
		if strings.HasPrefix(rel, "..") {
			continue
		}

		// Extract first path component (the ws-N directory name)
		parts := strings.SplitN(rel, string(filepath.Separator), 2)
		if len(parts) == 0 || parts[0] == "." || parts[0] == "" {
			continue
		}
		workspaceName := parts[0]
		workspacePath := filepath.Join(absRoot, workspaceName)

		info, err := os.Stat(workspacePath)
		if err != nil || !info.IsDir() {
			continue
		}
		if !IsInitialized(workspacePath) {
			continue
		}
		return profile, workspacePath, nil
	}
	return nil, "", nil
}
