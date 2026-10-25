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
