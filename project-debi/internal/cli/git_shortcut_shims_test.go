package cli

import (
	"devora/internal/shellinit"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

// TestGitShortcutShims_MatchesRegistry is the CI drift guard: it generates the shims from the live command registry and asserts the produced directory contains exactly one shim per "Git Shortcuts" command, each with the exact expected content and mode.
// Because expectations are derived from CommandInfos() itself, adding/removing/renaming a git shortcut keeps this honest and fails CI if shim generation drifts.
func TestGitShortcutShims_MatchesRegistry(t *testing.T) {
	dir := t.TempDir()
	if err := shellinit.WriteShims(dir, CommandInfos()); err != nil {
		t.Fatalf("WriteShims: %v", err)
	}

	var want []string
	for _, c := range CommandInfos() {
		if c.Group == shellinit.GitShortcutsGroup {
			want = append(want, c.Name)
		}
	}
	if len(want) == 0 {
		t.Fatal("registry reports no Git Shortcuts commands")
	}
	sort.Strings(want)

	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("read shim dir: %v", err)
	}
	var got []string
	for _, e := range entries {
		got = append(got, e.Name())
	}
	sort.Strings(got)

	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("shim set does not match registry\n got: %v\nwant: %v", got, want)
	}

	for _, name := range want {
		path := filepath.Join(dir, name)
		info, err := os.Stat(path)
		if err != nil {
			t.Fatalf("stat shim %q: %v", name, err)
		}
		if info.Mode().Perm() != 0o755 {
			t.Errorf("shim %q mode = %v, want 0755", name, info.Mode().Perm())
		}
		content, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("read shim %q: %v", name, err)
		}
		wantContent := "#!/bin/sh\nexec debi " + name + " \"$@\"\n"
		if string(content) != wantContent {
			t.Errorf("shim %q content = %q, want %q", name, string(content), wantContent)
		}
	}
}
