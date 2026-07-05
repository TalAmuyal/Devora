// Package shellinit materializes the shell integration shipped inside the Devora app bundle.
// Its only job today is to write "shims": one tiny executable per git shortcut alias that forwards to `debi git <target>`, so that typing the bare alias in any session shell (whose PATH includes the shim directory) runs the Debi command.
package shellinit

import (
	"fmt"
	"os"
	"path/filepath"
)

type ShimAlias struct {
	Name   string // shim filename, e.g. "gcl"
	Target string // args after `debi git `, e.g. "checkout-latest"
}

// GitShimAliases is the single source of truth for the shell shims.
// The alias NAMES are frozen external contracts - Judge's command allowlist contains "gaac" and Ember's e2e suite probes `command -v gcl` - so renames break integrations and muscle memory; additions are fine.
// The drift guard in internal/cli/git_shortcut_shims_test.go enforces this.
var GitShimAliases = []ShimAlias{
	{Name: "gaa", Target: "add ."},
	{Name: "gaaa", Target: "add-all-amend"},
	{Name: "gaaap", Target: "add-all-amend-push"},
	{Name: "gaac", Target: "add-all-commit"},
	{Name: "gaacp", Target: "add-all-commit-push"},
	{Name: "gb", Target: "branch"},
	{Name: "gbd", Target: "branch -D"},
	{Name: "gbdc", Target: "branch-delete-current"},
	{Name: "gcl", Target: "checkout-latest"},
	{Name: "gcom", Target: "checkout-origin-default"},
	{Name: "gd", Target: "diff"},
	{Name: "gfo", Target: "fetch origin"},
	{Name: "gg", Target: "grep"},
	{Name: "gl", Target: "log"},
	{Name: "gpo", Target: "push origin"},
	{Name: "gpof", Target: "push origin --force"},
	{Name: "gpop", Target: "stash pop"},
	{Name: "gri", Target: "rebase-interactive"},
	{Name: "grl", Target: "rebase-latest"},
	{Name: "grlp", Target: "rebase-latest --push"},
	{Name: "grom", Target: "rebase-origin-default"},
	{Name: "gst", Target: "status"},
	{Name: "gstash", Target: "stash"},
	{Name: "gsum", Target: "summary"},
}

func shimContent(target string) string {
	return fmt.Sprintf("#!/bin/sh\nexec debi git %s \"$@\"\n", target)
}

func WriteShims(dir string) error {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("create shim dir: %w", err)
	}

	for _, alias := range GitShimAliases {
		path := filepath.Join(dir, alias.Name)
		if err := os.WriteFile(path, []byte(shimContent(alias.Target)), 0o755); err != nil {
			return fmt.Errorf("write shim %s: %w", alias.Name, err)
		}
		// Chmod explicitly so the executable bits survive a restrictive umask.
		if err := os.Chmod(path, 0o755); err != nil {
			return fmt.Errorf("chmod shim %s: %w", alias.Name, err)
		}
	}

	return nil
}
