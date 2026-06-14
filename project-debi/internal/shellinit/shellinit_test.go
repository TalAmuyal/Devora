package shellinit

import (
	"devora/internal/cmdinfo"
	"os"
	"path/filepath"
	"testing"
)

func testCommands() []cmdinfo.Command {
	return []cmdinfo.Command{
		{Name: "gcl", Group: "Git Shortcuts", Description: "Fetch origin and checkout default branch"},
		{Name: "gst", Group: "Git Shortcuts", Description: "git status", ArgsHint: "[args]"},
		{Name: "health", Group: "Health", Description: "Check Devora dependencies"},
		{Name: "completion", Group: "Utility", Description: "Generate shell completion script"},
	}
}

func TestWriteShims_CreatesShimPerGitShortcut(t *testing.T) {
	dir := t.TempDir()
	if err := WriteShims(dir, testCommands()); err != nil {
		t.Fatalf("WriteShims: %v", err)
	}

	for _, name := range []string{"gcl", "gst"} {
		path := filepath.Join(dir, name)
		info, err := os.Stat(path)
		if err != nil {
			t.Fatalf("expected shim %q: %v", name, err)
		}
		if info.Mode().Perm() != 0o755 {
			t.Errorf("shim %q mode = %v, want 0755", name, info.Mode().Perm())
		}
		content, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("read shim %q: %v", name, err)
		}
		want := "#!/bin/sh\nexec debi " + name + " \"$@\"\n"
		if string(content) != want {
			t.Errorf("shim %q content = %q, want %q", name, string(content), want)
		}
	}
}

func TestWriteShims_SkipsNonGitCommands(t *testing.T) {
	dir := t.TempDir()
	if err := WriteShims(dir, testCommands()); err != nil {
		t.Fatalf("WriteShims: %v", err)
	}

	for _, name := range []string{"health", "completion"} {
		if _, err := os.Stat(filepath.Join(dir, name)); !os.IsNotExist(err) {
			t.Errorf("expected no shim for non-git command %q (stat err: %v)", name, err)
		}
	}
}

func TestWriteShims_CreatesMissingDir(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "nested", "git-shortcuts")
	if err := WriteShims(dir, testCommands()); err != nil {
		t.Fatalf("WriteShims: %v", err)
	}
	if _, err := os.Stat(filepath.Join(dir, "gcl")); err != nil {
		t.Fatalf("expected shim in created dir: %v", err)
	}
}
