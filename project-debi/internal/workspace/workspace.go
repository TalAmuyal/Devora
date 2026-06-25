package workspace

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"

	"devora/internal/config"
	"devora/internal/process"

	"golang.org/x/sync/errgroup"
)

const LockFileName = ".creation-lock"
const InitializedMarkerName = "initialized"
const TaskFileName = "task.json"
const ClaudeMDFileName = "CLAUDE.md"
const WorkspacePrefix = "ws-"

const WorkspaceCLAUDEMDContent = "This is a workspace with multiple repositories.\n" +
	"Run `find . -maxdepth 2 -name .git | sed 's|/\\.git$||'` to see which repos are available for work or reference.\n"

// --- Path Helpers ---

func GetWorkspaceRepoPath(workspacePath string, repoName string) string {
	return filepath.Join(workspacePath, repoName)
}

func GetWorkspaceTaskPath(workspacePath string) string {
	return filepath.Join(workspacePath, TaskFileName)
}

func getInitializationMarkerPath(workspacePath string) string {
	return filepath.Join(workspacePath, InitializedMarkerName)
}

func getLockPath(workspacePath string) string {
	return filepath.Join(workspacePath, LockFileName)
}

// --- File Locking ---

func LockWorkspace(workspacePath string) (io.Closer, error) {
	lockPath := getLockPath(workspacePath)
	f, err := os.OpenFile(lockPath, os.O_WRONLY|os.O_CREATE, 0666)
	if err != nil {
		return nil, err
	}
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX); err != nil {
		f.Close()
		return nil, err
	}
	return f, nil
}

func IsWorkspaceLocked(workspacePath string) bool {
	lockPath := getLockPath(workspacePath)
	f, err := os.Open(lockPath)
	if err != nil {
		return false
	}
	defer f.Close()

	err = syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB)
	if err == syscall.EWOULDBLOCK {
		return true
	}
	if err != nil {
		return false
	}
	// Lock acquired successfully, release it
	syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
	return false
}

// --- State Query Functions ---

func IsInitialized(workspacePath string) bool {
	_, err := os.Stat(getInitializationMarkerPath(workspacePath))
	return err == nil
}

func HasTask(workspacePath string) bool {
	_, err := os.Stat(GetWorkspaceTaskPath(workspacePath))
	return err == nil
}

func IsInactive(workspacePath string) bool {
	return IsInitialized(workspacePath) && !HasTask(workspacePath)
}

// --- Workspace Enumeration ---

func GetWorkspaces(workspacesRoot string) ([]string, error) {
	entries, err := os.ReadDir(workspacesRoot)
	if err != nil {
		if os.IsNotExist(err) {
			return []string{}, nil
		}
		return nil, err
	}

	var workspaces []string
	for _, entry := range entries {
		if entry.IsDir() && strings.HasPrefix(entry.Name(), WorkspacePrefix) {
			workspaces = append(workspaces, filepath.Join(workspacesRoot, entry.Name()))
		}
	}
	return workspaces, nil
}

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

// --- Workspace Search ---

func SearchAvailableWorkspace(repos []string, workspacesRoot string) (string, error) {
	workspaces, err := GetWorkspaces(workspacesRoot)
	if err != nil {
		return "", err
	}

	sortedRepos := make([]string, len(repos))
	copy(sortedRepos, repos)
	sort.Strings(sortedRepos)

	for _, ws := range workspaces {
		if IsWorkspaceLocked(ws) {
			continue
		}
		if !IsInactive(ws) {
			continue
		}
		wsRepos, err := GetWorkspaceRepos(ws)
		if err != nil {
			continue
		}
		sort.Strings(wsRepos)
		if slicesEqual(sortedRepos, wsRepos) {
			return ws, nil
		}
	}
	return "", nil
}

func slicesEqual(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// --- Workspace Creation ---

func CreateWorkspaceDirectory(workspacesRoot string) (string, error) {
	for n := 1; ; n++ {
		wsPath := filepath.Join(workspacesRoot, fmt.Sprintf("ws-%d", n))
		if _, err := os.Stat(wsPath); os.IsNotExist(err) {
			if err := os.MkdirAll(wsPath, 0o755); err != nil {
				return "", err
			}
			return wsPath, nil
		}
	}
}

func MarkInitialized(workspacePath string) error {
	return os.WriteFile(getInitializationMarkerPath(workspacePath), []byte{}, 0666)
}

// --- CLAUDE.md Management ---

func WriteWorkspaceCLAUDEMD(workspacePath string) error {
	return os.WriteFile(filepath.Join(workspacePath, ClaudeMDFileName), []byte(WorkspaceCLAUDEMDContent), 0666)
}

// --- Workspace Deactivation ---

func DeactivateWorkspace(workspacePath string) error {
	taskPath := GetWorkspaceTaskPath(workspacePath)
	err := os.Remove(taskPath)
	if os.IsNotExist(err) {
		return nil
	}
	return err
}

// --- Workspace Deletion ---

func DeleteWorkspace(workspacePath string) error {
	repos, err := GetWorkspaceRepos(workspacePath)
	if err != nil {
		return err
	}

	g, _ := errgroup.WithContext(context.Background())
	for _, repo := range repos {
		g.Go(func() error {
			_, err := process.GetOutput(
				[]string{"git", "worktree", "remove", "."},
				process.WithCwd(GetWorkspaceRepoPath(workspacePath, repo)),
			)
			return err
		})
	}
	if err := g.Wait(); err != nil {
		return err
	}
	return os.RemoveAll(workspacePath)
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

// --- Session Working Directory ---

func GetSessionWorkingDirectory(workspacePath string) (string, error) {
	repos, err := GetWorkspaceRepos(workspacePath)
	if err != nil {
		return "", err
	}
	if len(repos) == 1 {
		return GetWorkspaceRepoPath(workspacePath, repos[0]), nil
	}
	return workspacePath, nil
}
