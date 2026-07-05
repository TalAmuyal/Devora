package git

import (
	"os"
	"path/filepath"
	"strings"
)

func ChildRepos(base string) ([]string, error) {
	entries, err := os.ReadDir(base)
	if err != nil {
		return nil, err
	}
	var repos []string
	for _, entry := range entries {
		if !entry.IsDir() || strings.HasPrefix(entry.Name(), ".") {
			continue
		}
		if _, err := os.Stat(filepath.Join(base, entry.Name(), ".git")); err == nil {
			repos = append(repos, entry.Name())
		}
	}
	return repos, nil
}
