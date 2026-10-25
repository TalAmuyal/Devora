package workspace

import (
	"devora/internal/process"
)

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
