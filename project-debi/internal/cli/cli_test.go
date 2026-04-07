package cli

import (
	"bytes"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRun_HelpFlags_PrintsUsageAndReturnsNil(t *testing.T) {
	for _, flag := range []string{"-h", "--help"} {
		t.Run(flag, func(t *testing.T) {
			old := os.Stdout
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			os.Stdout = w

			runErr := Run([]string{flag})

			w.Close()
			os.Stdout = old

			if runErr != nil {
				t.Fatalf("expected no error for %s, got: %s", flag, runErr.Error())
			}

			var buf bytes.Buffer
			io.Copy(&buf, r)
			output := buf.String()
			if !strings.Contains(output, "usage: debi") {
				t.Fatalf("expected usage message on stdout, got: %q", output)
			}
		})
	}
}

func TestRun_NoArgs_ReturnsUsageError(t *testing.T) {
	err := Run([]string{})
	if err == nil {
		t.Fatal("expected error for empty args")
	}
	if !strings.Contains(err.Error(), "usage") {
		t.Fatalf("expected usage message, got: %s", err.Error())
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_UnknownCommand_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"nonexistent"})
	if err == nil {
		t.Fatal("expected error for unknown command")
	}
	if !strings.Contains(err.Error(), "nonexistent") {
		t.Fatalf("expected error to mention command name, got: %s", err.Error())
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_RenameMissingArg_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"rename"})
	if err == nil {
		t.Fatal("expected error for rename without arg")
	}
	if !strings.Contains(err.Error(), "usage") {
		t.Fatalf("expected usage message, got: %s", err.Error())
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_RenameAlias_MissingArg_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"r"})
	if err == nil {
		t.Fatal("expected error for rename alias without arg")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

// workspace-ui and add commands attempt real operations.
// In test environments (no TTY, not inside a workspace), they produce
// expected errors rather than "not yet implemented" messages.

func TestRun_WorkspaceUI_Recognized(t *testing.T) {
	err := Run([]string{"workspace-ui"})
	// May fail due to TTY not available in tests, but should not return "unknown command"
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("workspace-ui should be recognized, got: %s", err.Error())
	}
}

func TestRun_WorkspaceUI_Alias(t *testing.T) {
	err := Run([]string{"w"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("w alias should be recognized, got: %s", err.Error())
	}
}

func TestRun_Add_Recognized(t *testing.T) {
	// Chdir to a temp directory so we are not inside a known workspace
	origDir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(origDir) })
	os.Chdir(t.TempDir())

	err = Run([]string{"add"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("add should be recognized, got: %s", err.Error())
	}
	// Expected: "not inside a known workspace" since tests don't run from a workspace
	if err != nil && !strings.Contains(err.Error(), "workspace") {
		t.Fatalf("expected workspace-related error, got: %s", err.Error())
	}
}

func TestRun_Add_Alias(t *testing.T) {
	origDir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(origDir) })
	os.Chdir(t.TempDir())

	err = Run([]string{"a"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("a alias should be recognized, got: %s", err.Error())
	}
}

func TestRun_Health_Recognized(t *testing.T) {
	err := Run([]string{"health"})
	// May fail due to missing deps, but should not return "unknown command"
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("health should be recognized, got: %s", err.Error())
	}
}

func TestRun_Health_Help(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"health", "--help"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for health --help, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "usage: debi health") {
		t.Fatalf("expected health usage message on stdout, got: %q", output)
	}
}

func TestRun_Health_UnknownFlag(t *testing.T) {
	err := Run([]string{"health", "--foo"})
	if err == nil {
		t.Fatal("expected error for unknown flag")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "--foo") {
		t.Fatalf("expected error to mention the flag, got: %s", err.Error())
	}
}

func TestRun_GitCommands_Recognized(t *testing.T) {
	origDir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(origDir) })
	os.Chdir(t.TempDir())

	commands := []string{
		"gaa", "gaaa", "gaaap", "gb", "gbdc", "gcl", "gcom", "gd",
		"gfo", "gg", "gl", "gpo", "gpof", "gpop", "gri",
		"grl", "grlp", "grom", "gst", "gstash",
	}
	for _, cmd := range commands {
		t.Run(cmd, func(t *testing.T) {
			err := Run([]string{cmd})
			if err != nil && strings.Contains(err.Error(), "unknown command") {
				t.Fatalf("%s should be recognized, got: %s", cmd, err.Error())
			}
		})
	}
}

func TestRun_GitCommandsWithArgs_Recognized(t *testing.T) {
	origDir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(origDir) })
	os.Chdir(t.TempDir())

	commands := []struct {
		args []string
	}{
		{[]string{"gaac", "test message"}},
		{[]string{"gaacp", "test message"}},
		{[]string{"gbd", "some-branch"}},
	}
	for _, tc := range commands {
		t.Run(tc.args[0], func(t *testing.T) {
			err := Run(tc.args)
			if err != nil && strings.Contains(err.Error(), "unknown command") {
				t.Fatalf("%s should be recognized, got: %s", tc.args[0], err.Error())
			}
		})
	}
}

func TestRun_Gaac_MissingArg_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"gaac"})
	if err == nil {
		t.Fatal("expected error for gaac without arg")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_Gaacp_MissingArg_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"gaacp"})
	if err == nil {
		t.Fatal("expected error for gaacp without arg")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_Gbd_MissingArg_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"gbd"})
	if err == nil {
		t.Fatal("expected error for gbd without arg")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestCheckBundledAppsInPath_EmptyResourcesDir_ReturnsEmpty(t *testing.T) {
	result := checkBundledAppsInPath("", "/usr/bin:/usr/local/bin")
	if result != "" {
		t.Fatalf("expected empty string for empty resourcesDir, got: %q", result)
	}
}

func TestCheckBundledAppsInPath_BundledAppsPresent_ReturnsEmpty(t *testing.T) {
	resourcesDir := "/Applications/Devora.app/Contents/Resources"
	bundledApps := filepath.Join(resourcesDir, "bundled-apps")
	pathEnv := "/usr/bin:" + bundledApps + ":/usr/local/bin"

	result := checkBundledAppsInPath(resourcesDir, pathEnv)
	if result != "" {
		t.Fatalf("expected empty string when bundled-apps is in PATH, got: %q", result)
	}
}

func TestCheckBundledAppsInPath_BundledAppsMissing_ReturnsWarning(t *testing.T) {
	resourcesDir := "/Applications/Devora.app/Contents/Resources"
	pathEnv := "/usr/bin:/usr/local/bin"

	result := checkBundledAppsInPath(resourcesDir, pathEnv)
	if result == "" {
		t.Fatal("expected non-empty warning when bundled-apps is not in PATH")
	}
}

func TestCheckBundledAppsInPath_TrailingSlash_ReturnsEmpty(t *testing.T) {
	resourcesDir := "/Applications/Devora.app/Contents/Resources"
	bundledAppsWithSlash := filepath.Join(resourcesDir, "bundled-apps") + "/"
	pathEnv := "/usr/bin:" + bundledAppsWithSlash + ":/usr/local/bin"

	result := checkBundledAppsInPath(resourcesDir, pathEnv)
	if result != "" {
		t.Fatalf("expected empty string when bundled-apps with trailing slash is in PATH, got: %q", result)
	}
}

func TestCheckBundledAppsInPath_MultipleEntries_BundledAppsPresent_ReturnsEmpty(t *testing.T) {
	resourcesDir := "/Applications/Devora.app/Contents/Resources"
	bundledApps := filepath.Join(resourcesDir, "bundled-apps")
	pathEnv := "/usr/bin:/opt/homebrew/bin:" + bundledApps + ":/usr/local/bin:/home/user/.local/bin"

	result := checkBundledAppsInPath(resourcesDir, pathEnv)
	if result != "" {
		t.Fatalf("expected empty string when bundled-apps is among multiple PATH entries, got: %q", result)
	}
}

func TestFormatStartupError_ContainsExpectedDirectory(t *testing.T) {
	expectedDir := "/Applications/Devora.app/Contents/Resources/bundled-apps"
	pathEnv := "/usr/bin:/usr/local/bin"

	result := formatStartupError(expectedDir, pathEnv)
	if !strings.Contains(result, expectedDir) {
		t.Fatalf("expected output to contain the expected directory %q, got: %q", expectedDir, result)
	}
}

func TestFormatStartupError_ContainsPathEntries(t *testing.T) {
	expectedDir := "/Applications/Devora.app/Contents/Resources/bundled-apps"
	pathEnv := "/usr/bin:/usr/local/bin"

	result := formatStartupError(expectedDir, pathEnv)
	if !strings.Contains(result, "/usr/bin") {
		t.Fatalf("expected output to contain PATH entry /usr/bin, got: %q", result)
	}
	if !strings.Contains(result, "/usr/local/bin") {
		t.Fatalf("expected output to contain PATH entry /usr/local/bin, got: %q", result)
	}
}

func TestFormatStartupError_ContainsDiagnosticDataOnly(t *testing.T) {
	expectedDir := "/Applications/Devora.app/Contents/Resources/bundled-apps"
	pathEnv := "/usr/bin:/usr/local/bin"

	result := formatStartupError(expectedDir, pathEnv)
	if strings.Contains(result, ".zprofile") || strings.Contains(result, ".zshrc") {
		t.Fatalf("formatStartupError should not contain fix instructions (owned by TUI layer), got: %q", result)
	}
}
