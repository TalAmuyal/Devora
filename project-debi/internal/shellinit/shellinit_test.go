package shellinit

import (
	"os"
	"path/filepath"
	"testing"
)

func TestWriteShims_CreatesShimPerAlias(t *testing.T) {
	dir := t.TempDir()
	if err := WriteShims(dir); err != nil {
		t.Fatalf("WriteShims: %v", err)
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("read shim dir: %v", err)
	}
	if len(entries) != len(GitShimAliases) {
		t.Fatalf("shim count = %d, want %d", len(entries), len(GitShimAliases))
	}

	for _, alias := range GitShimAliases {
		path := filepath.Join(dir, alias.Name)
		info, err := os.Stat(path)
		if err != nil {
			t.Fatalf("expected shim %q: %v", alias.Name, err)
		}
		if info.Mode().Perm() != 0o755 {
			t.Errorf("shim %q mode = %v, want 0755", alias.Name, info.Mode().Perm())
		}
		content, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("read shim %q: %v", alias.Name, err)
		}
		want := "#!/bin/sh\nexec debi git " + alias.Target + " \"$@\"\n"
		if string(content) != want {
			t.Errorf("shim %q content = %q, want %q", alias.Name, string(content), want)
		}
	}
}

func TestWriteShims_KnownAliasTargets(t *testing.T) {
	targets := make(map[string]string, len(GitShimAliases))
	for _, alias := range GitShimAliases {
		targets[alias.Name] = alias.Target
	}

	for name, want := range map[string]string{
		"gaa":  "add .",
		"gcl":  "checkout-latest",
		"gst":  "status",
		"gpof": "push origin --force",
		"gpop": "stash pop",
		"grlp": "rebase-latest --push",
		"gsum": "summary",
	} {
		if targets[name] != want {
			t.Errorf("alias %q target = %q, want %q", name, targets[name], want)
		}
	}
}

func TestWriteShims_CreatesMissingDir(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "nested", "git-shortcuts")
	if err := WriteShims(dir); err != nil {
		t.Fatalf("WriteShims: %v", err)
	}
	if _, err := os.Stat(filepath.Join(dir, "gcl")); err != nil {
		t.Fatalf("expected shim in created dir: %v", err)
	}
}
