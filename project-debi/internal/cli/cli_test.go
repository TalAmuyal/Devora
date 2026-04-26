package cli

import (
	"bytes"
	closecmd "devora/internal/close"
	"devora/internal/credentials"
	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/submit"
	"devora/internal/workspace/wsgit"
	"errors"
	"fmt"
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

func TestRun_Health_Recognized(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubHealthRun(t, func(io.Writer, bool, bool) error { return nil })
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

func TestRun_Health_ProfileSpaceSyntax_PassedToResolver(t *testing.T) {
	captured := ""
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		captured = explicit
		return explicit, nil
	})
	stubHealthRun(t, func(io.Writer, bool, bool) error { return nil })

	err := Run([]string{"health", "--profile", "work"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "work" {
		t.Fatalf("expected resolver called with 'work', got %q", captured)
	}
}

func TestRun_Health_ProfileEqualsSyntax_PassedToResolver(t *testing.T) {
	captured := ""
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		captured = explicit
		return explicit, nil
	})
	stubHealthRun(t, func(io.Writer, bool, bool) error { return nil })

	err := Run([]string{"health", "--profile=work"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "work" {
		t.Fatalf("expected resolver called with 'work', got %q", captured)
	}
}

func TestRun_Health_ProfileShortFlag_PassedToResolver(t *testing.T) {
	captured := ""
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		captured = explicit
		return explicit, nil
	})
	stubHealthRun(t, func(io.Writer, bool, bool) error { return nil })

	err := Run([]string{"health", "-p", "work"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "work" {
		t.Fatalf("expected resolver called with 'work', got %q", captured)
	}
}

func TestRun_Health_ProfileMissingValue_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"health", "--profile"})
	if err == nil {
		t.Fatal("expected error for --profile without value")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "--profile") {
		t.Fatalf("expected error to mention --profile, got: %s", err.Error())
	}
}

func TestRun_Health_ProfileNotFound_ReturnsUsageError_SkipsHealthRun(t *testing.T) {
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		return "", &UsageError{Message: "profile not found: nope"}
	})
	stubHealthRun(t, func(io.Writer, bool, bool) error {
		t.Fatal("healthRun should not run when resolver returns UsageError")
		return nil
	})

	err := Run([]string{"health", "--profile", "nope"})
	if err == nil {
		t.Fatal("expected error for unknown profile")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "profile not found") {
		t.Fatalf("expected 'profile not found' message, got: %s", err.Error())
	}
}

func TestRun_Health_ResolverInvokedBeforeRun(t *testing.T) {
	// Guard against regressions where a future refactor calls healthRun
	// before the profile resolver. The resolver must run first so profile-
	// level tracker config becomes visible.
	order := []string{}
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		order = append(order, "resolve")
		return "", nil
	})
	stubHealthRun(t, func(io.Writer, bool, bool) error {
		order = append(order, "health")
		return nil
	})

	err := Run([]string{"health"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if len(order) != 2 || order[0] != "resolve" || order[1] != "health" {
		t.Fatalf("expected resolver before health, got order: %v", order)
	}
}

func TestRun_Submit_ResolverInvokedBeforeRun(t *testing.T) {
	order := []string{}
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		order = append(order, "resolve")
		return "", nil
	})
	stubSubmitRun(t, func(io.Writer, submit.Options) error {
		order = append(order, "submit")
		return nil
	})

	err := Run([]string{"pr", "submit", "-m", "msg"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if len(order) != 2 || order[0] != "resolve" || order[1] != "submit" {
		t.Fatalf("expected resolver before submit, got order: %v", order)
	}
}

func TestRun_Close_ResolverInvokedBeforeRun(t *testing.T) {
	order := []string{}
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		order = append(order, "resolve")
		return "", nil
	})
	stubCloseRun(t, func(io.Writer, closecmd.Options) error {
		order = append(order, "close")
		return nil
	})

	err := Run([]string{"pr", "close"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if len(order) != 2 || order[0] != "resolve" || order[1] != "close" {
		t.Fatalf("expected resolver before close, got order: %v", order)
	}
}

func TestRun_Submit_ResolverError_SkipsSubmit(t *testing.T) {
	// If the resolver fails (e.g., an unknown explicit profile — not a
	// submit path today, but guard the contract), submit must not run.
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		return "", errors.New("resolver failed")
	})
	stubSubmitRun(t, func(io.Writer, submit.Options) error {
		t.Fatal("submit should not run after resolver error")
		return nil
	})

	err := Run([]string{"pr", "submit", "-m", "msg"})
	if err == nil {
		t.Fatal("expected error when resolver fails")
	}
	if !strings.Contains(err.Error(), "resolver failed") {
		t.Fatalf("expected resolver error to bubble, got: %s", err.Error())
	}
}

func TestRun_Close_ResolverError_SkipsClose(t *testing.T) {
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		return "", errors.New("resolver failed")
	})
	stubCloseRun(t, func(io.Writer, closecmd.Options) error {
		t.Fatal("close should not run after resolver error")
		return nil
	})

	err := Run([]string{"pr", "close"})
	if err == nil {
		t.Fatal("expected error when resolver fails")
	}
	if !strings.Contains(err.Error(), "resolver failed") {
		t.Fatalf("expected resolver error to bubble, got: %s", err.Error())
	}
}

func TestRun_PR_MissingSubcommand_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr"})
	if err == nil {
		t.Fatal("expected error for pr without subcommand")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_PR_UnknownSubcommand_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "foo"})
	if err == nil {
		t.Fatal("expected error for unknown pr subcommand")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "foo") {
		t.Fatalf("expected error to mention subcommand name, got: %s", err.Error())
	}
}

func TestRun_PR_Help_PrintsUsage(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"pr", "-h"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for pr -h, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "usage: debi pr") {
		t.Fatalf("expected pr usage message on stdout, got: %q", output)
	}
}

func TestRun_PRCheck_Help_PrintsUsage(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"pr", "check", "-h"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for pr check -h, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "usage: debi pr check") {
		t.Fatalf("expected pr check usage message on stdout, got: %q", output)
	}
}

func TestRun_PRCheck_UnknownFlag_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "check", "--foo"})
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

func TestUsageMessage_ContainsAllCommands(t *testing.T) {
	msg := usageMessage()

	// Verify header
	if !strings.HasPrefix(msg, "usage: debi <command> [args]") {
		t.Fatalf("usage message should start with header, got: %q", msg[:50])
	}

	// Verify all group headers
	for _, group := range groupOrder {
		if !strings.Contains(msg, group+":") {
			t.Fatalf("usage message should contain group header %q", group+":")
		}
	}

	// Verify all command names appear
	for _, cmd := range commands {
		if !strings.Contains(msg, cmd.Name) {
			t.Fatalf("usage message should contain command %q", cmd.Name)
		}
	}
}

func TestRun_Util_MissingSubcommand_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"util"})
	if err == nil {
		t.Fatal("expected error for util without subcommand")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_Util_UnknownSubcommand_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"util", "foo"})
	if err == nil {
		t.Fatal("expected error for unknown util subcommand")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "foo") {
		t.Fatalf("expected error to mention subcommand name, got: %s", err.Error())
	}
}

func TestRun_Util_Help_PrintsUsage(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "-h"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for util -h, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "usage: debi util") {
		t.Fatalf("expected util usage message on stdout, got: %q", output)
	}
	for _, name := range []string{"json-validate", "yaml-validate", "toml-validate"} {
		if !strings.Contains(output, name) {
			t.Fatalf("expected util help to mention %q, got: %q", name, output)
		}
	}
}

func TestRun_JSONValidate_NoArgs_ReturnsPassthroughError2(t *testing.T) {
	// Capture stderr since runJSONValidate prints usage there
	oldStderr := os.Stderr
	_, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() {
		w.Close()
		os.Stderr = oldStderr
	}()

	runErr := Run([]string{"util", "json-validate"})
	if runErr == nil {
		t.Fatal("expected error for json-validate without args")
	}
	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", runErr, runErr.Error())
	}
	if ptErr.Code != 2 {
		t.Fatalf("expected exit code 2, got: %d", ptErr.Code)
	}
}

func TestRun_JSONValidate_ValidFile_PrintsValidJSON(t *testing.T) {
	tmpFile := filepath.Join(t.TempDir(), "valid.json")
	if err := os.WriteFile(tmpFile, []byte(`{"key": "value"}`), 0644); err != nil {
		t.Fatal(err)
	}

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "json-validate", tmpFile})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for valid JSON file, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Valid JSON") {
		t.Fatalf("expected 'Valid JSON' on stdout, got: %q", output)
	}
}

func TestRun_JSONValidate_InvalidFile_ReturnsPassthroughError1(t *testing.T) {
	tmpFile := filepath.Join(t.TempDir(), "invalid.json")
	if err := os.WriteFile(tmpFile, []byte(`{"bad": }`), 0644); err != nil {
		t.Fatal(err)
	}

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "json-validate", tmpFile})

	w.Close()
	os.Stdout = old

	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %v", runErr, runErr)
	}
	if ptErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", ptErr.Code)
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Invalid JSON") {
		t.Fatalf("expected 'Invalid JSON' on stdout, got: %q", output)
	}
}

func TestRun_JSONValidate_Stdin_ValidJSON(t *testing.T) {
	stdinR, stdinW, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	stdinW.Write([]byte(`{"key": "value"}`))
	stdinW.Close()

	oldStdin := os.Stdin
	os.Stdin = stdinR
	defer func() { os.Stdin = oldStdin }()

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "json-validate", "-"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for valid JSON on stdin, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Valid JSON") {
		t.Fatalf("expected 'Valid JSON' on stdout, got: %q", output)
	}
}

func TestRun_JSONValidate_NonexistentFile_ReturnsPassthroughError2(t *testing.T) {
	// Capture stderr since runJSONValidate prints the error there
	oldStderr := os.Stderr
	_, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() {
		w.Close()
		os.Stderr = oldStderr
	}()

	runErr := Run([]string{"util", "json-validate", "/tmp/nonexistent-json-validate-test-file.json"})
	if runErr == nil {
		t.Fatal("expected error for nonexistent file")
	}
	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", runErr, runErr.Error())
	}
	if ptErr.Code != 2 {
		t.Fatalf("expected exit code 2, got: %d", ptErr.Code)
	}
}

func TestRun_YAMLValidate_NoArgs_ReturnsPassthroughError2(t *testing.T) {
	// Capture stderr since runYAMLValidate prints usage there
	oldStderr := os.Stderr
	_, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() {
		w.Close()
		os.Stderr = oldStderr
	}()

	runErr := Run([]string{"util", "yaml-validate"})
	if runErr == nil {
		t.Fatal("expected error for yaml-validate without args")
	}
	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", runErr, runErr.Error())
	}
	if ptErr.Code != 2 {
		t.Fatalf("expected exit code 2, got: %d", ptErr.Code)
	}
}

func TestRun_YAMLValidate_ValidFile_PrintsValidYAML(t *testing.T) {
	tmpFile := filepath.Join(t.TempDir(), "valid.yaml")
	if err := os.WriteFile(tmpFile, []byte("key: value\nother: 7\n"), 0644); err != nil {
		t.Fatal(err)
	}

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "yaml-validate", tmpFile})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for valid YAML file, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Valid YAML") {
		t.Fatalf("expected 'Valid YAML' on stdout, got: %q", output)
	}
}

func TestRun_YAMLValidate_InvalidFile_ReturnsPassthroughError1(t *testing.T) {
	tmpFile := filepath.Join(t.TempDir(), "invalid.yaml")
	if err := os.WriteFile(tmpFile, []byte("{key: value"), 0644); err != nil {
		t.Fatal(err)
	}

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "yaml-validate", tmpFile})

	w.Close()
	os.Stdout = old

	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %v", runErr, runErr)
	}
	if ptErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", ptErr.Code)
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Invalid YAML") {
		t.Fatalf("expected 'Invalid YAML' on stdout, got: %q", output)
	}
}

func TestRun_YAMLValidate_Stdin_ValidYAML(t *testing.T) {
	stdinR, stdinW, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	stdinW.Write([]byte("key: value\n"))
	stdinW.Close()

	oldStdin := os.Stdin
	os.Stdin = stdinR
	defer func() { os.Stdin = oldStdin }()

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "yaml-validate", "-"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for valid YAML on stdin, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Valid YAML") {
		t.Fatalf("expected 'Valid YAML' on stdout, got: %q", output)
	}
}

func TestRun_YAMLValidate_NonexistentFile_ReturnsPassthroughError2(t *testing.T) {
	// Capture stderr since runYAMLValidate prints the error there
	oldStderr := os.Stderr
	_, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() {
		w.Close()
		os.Stderr = oldStderr
	}()

	runErr := Run([]string{"util", "yaml-validate", "/tmp/nonexistent-yaml-validate-test-file.yaml"})
	if runErr == nil {
		t.Fatal("expected error for nonexistent file")
	}
	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", runErr, runErr.Error())
	}
	if ptErr.Code != 2 {
		t.Fatalf("expected exit code 2, got: %d", ptErr.Code)
	}
}

func TestRun_TOMLValidate_NoArgs_ReturnsPassthroughError2(t *testing.T) {
	// Capture stderr since runTOMLValidate prints usage there
	oldStderr := os.Stderr
	_, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() {
		w.Close()
		os.Stderr = oldStderr
	}()

	runErr := Run([]string{"util", "toml-validate"})
	if runErr == nil {
		t.Fatal("expected error for toml-validate without args")
	}
	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", runErr, runErr.Error())
	}
	if ptErr.Code != 2 {
		t.Fatalf("expected exit code 2, got: %d", ptErr.Code)
	}
}

func TestRun_TOMLValidate_ValidFile_PrintsValidTOML(t *testing.T) {
	tmpFile := filepath.Join(t.TempDir(), "valid.toml")
	if err := os.WriteFile(tmpFile, []byte(`title = "test"`+"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "toml-validate", tmpFile})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for valid TOML file, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Valid TOML") {
		t.Fatalf("expected 'Valid TOML' on stdout, got: %q", output)
	}
}

func TestRun_TOMLValidate_InvalidFile_ReturnsPassthroughError1(t *testing.T) {
	tmpFile := filepath.Join(t.TempDir(), "invalid.toml")
	if err := os.WriteFile(tmpFile, []byte("a = 1\na = 2\n"), 0644); err != nil {
		t.Fatal(err)
	}

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "toml-validate", tmpFile})

	w.Close()
	os.Stdout = old

	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %v", runErr, runErr)
	}
	if ptErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", ptErr.Code)
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Invalid TOML") {
		t.Fatalf("expected 'Invalid TOML' on stdout, got: %q", output)
	}
}

func TestRun_TOMLValidate_Stdin_ValidTOML(t *testing.T) {
	stdinR, stdinW, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	stdinW.Write([]byte(`title = "test"` + "\n"))
	stdinW.Close()

	oldStdin := os.Stdin
	os.Stdin = stdinR
	defer func() { os.Stdin = oldStdin }()

	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"util", "toml-validate", "-"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for valid TOML on stdin, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Valid TOML") {
		t.Fatalf("expected 'Valid TOML' on stdout, got: %q", output)
	}
}

func TestRun_TOMLValidate_NonexistentFile_ReturnsPassthroughError2(t *testing.T) {
	// Capture stderr since runTOMLValidate prints the error there
	oldStderr := os.Stderr
	_, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() {
		w.Close()
		os.Stderr = oldStderr
	}()

	runErr := Run([]string{"util", "toml-validate", "/tmp/nonexistent-toml-validate-test-file.toml"})
	if runErr == nil {
		t.Fatal("expected error for nonexistent file")
	}
	var ptErr *process.PassthroughError
	if !errors.As(runErr, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", runErr, runErr.Error())
	}
	if ptErr.Code != 2 {
		t.Fatalf("expected exit code 2, got: %d", ptErr.Code)
	}
}

func TestRun_Completion_Recognized(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"completion", "bash"})

	w.Close()
	os.Stdout = old
	io.Copy(io.Discard, r)

	if runErr != nil && strings.Contains(runErr.Error(), "unknown command") {
		t.Fatalf("completion should be recognized, got: %s", runErr.Error())
	}
	if runErr != nil {
		t.Fatalf("expected no error for completion bash, got: %s", runErr.Error())
	}
}

func TestRun_Completion_MissingArg_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"completion"})
	if err == nil {
		t.Fatal("expected error for completion without arg")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_Completion_InvalidShell_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"completion", "powershell"})
	if err == nil {
		t.Fatal("expected error for unsupported shell")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "unsupported shell") {
		t.Fatalf("expected error to mention 'unsupported shell', got: %s", err.Error())
	}
}

func TestRun_Completion_ProducesOutput(t *testing.T) {
	tests := []struct {
		shell    string
		contains string
	}{
		{shell: "bash", contains: ""},
		{shell: "zsh", contains: "#compdef"},
		{shell: "fish", contains: "complete -c"},
	}
	for _, tc := range tests {
		t.Run(tc.shell, func(t *testing.T) {
			old := os.Stdout
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			os.Stdout = w

			runErr := Run([]string{"completion", tc.shell})

			w.Close()
			os.Stdout = old

			if runErr != nil {
				t.Fatalf("expected no error, got: %s", runErr.Error())
			}

			var buf bytes.Buffer
			io.Copy(&buf, r)
			output := buf.String()
			if len(output) == 0 {
				t.Fatalf("expected non-empty %s completion output", tc.shell)
			}
			if tc.contains != "" && !strings.Contains(output, tc.contains) {
				t.Fatalf("expected %s completion output to contain %q, got: %q", tc.shell, tc.contains, output)
			}
		})
	}
}

func TestRun_Completion_Help_PrintsGeneralHelp(t *testing.T) {
	for _, flag := range []string{"-h", "--help"} {
		t.Run(flag, func(t *testing.T) {
			old := os.Stdout
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			os.Stdout = w

			runErr := Run([]string{"completion", flag})

			w.Close()
			os.Stdout = old

			if runErr != nil {
				t.Fatalf("expected no error for completion %s, got: %s", flag, runErr.Error())
			}

			var buf bytes.Buffer
			io.Copy(&buf, r)
			output := buf.String()
			if !strings.Contains(output, "Usage: debi completion <bash|zsh|fish>") {
				t.Fatalf("expected general usage line in output, got: %q", output)
			}
			if !strings.Contains(output, "shell-specific installation instructions") {
				t.Fatalf("expected shell-specific hint in output, got: %q", output)
			}
		})
	}
}

func TestRun_Completion_ShellHelp_PrintsShellSpecificHelp(t *testing.T) {
	tests := []struct {
		args     []string
		contains string
	}{
		{args: []string{"completion", "bash", "-h"}, contains: "source <(debi completion bash)"},
		{args: []string{"completion", "-h", "bash"}, contains: "source <(debi completion bash)"},
		{args: []string{"completion", "bash", "--help"}, contains: "source <(debi completion bash)"},
		{args: []string{"completion", "zsh", "-h"}, contains: "source <(debi completion zsh)"},
		{args: []string{"completion", "zsh", "--help"}, contains: `~/.zsh/completions/_debi`},
		{args: []string{"completion", "fish", "-h"}, contains: "debi completion fish | source"},
		{args: []string{"completion", "fish", "--help"}, contains: "completions/debi.fish"},
	}
	for _, tc := range tests {
		t.Run(strings.Join(tc.args, " "), func(t *testing.T) {
			old := os.Stdout
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			os.Stdout = w

			runErr := Run(tc.args)

			w.Close()
			os.Stdout = old

			if runErr != nil {
				t.Fatalf("expected no error, got: %s", runErr.Error())
			}

			var buf bytes.Buffer
			io.Copy(&buf, r)
			output := buf.String()
			if !strings.Contains(output, tc.contains) {
				t.Fatalf("expected output to contain %q, got: %q", tc.contains, output)
			}
		})
	}
}

func TestRun_Completion_InvalidShellWithHelp_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"completion", "powershell", "-h"})
	if err == nil {
		t.Fatal("expected error for unsupported shell with help flag")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "unsupported shell") {
		t.Fatalf("expected error to mention 'unsupported shell', got: %s", err.Error())
	}
}

func TestCommandIndex_AllCommandsResolvable(t *testing.T) {
	for _, cmd := range commands {
		resolved, ok := commandIndex[cmd.Name]
		if !ok {
			t.Fatalf("command %q not found in commandIndex", cmd.Name)
		}
		if resolved.Name != cmd.Name {
			t.Fatalf("commandIndex[%q] resolved to %q", cmd.Name, resolved.Name)
		}

		if cmd.Alias != "" {
			resolved, ok := commandIndex[cmd.Alias]
			if !ok {
				t.Fatalf("alias %q for command %q not found in commandIndex", cmd.Alias, cmd.Name)
			}
			if resolved.Name != cmd.Name {
				t.Fatalf("commandIndex[%q] resolved to %q, expected %q", cmd.Alias, resolved.Name, cmd.Name)
			}
		}
	}
}

// stubSubmitRun overrides submitRun for the duration of the test.
func stubSubmitRun(t *testing.T, fn func(w io.Writer, opts submit.Options) error) {
	t.Helper()
	orig := submitRun
	submitRun = fn
	t.Cleanup(func() { submitRun = orig })
}

// stubCloseRun overrides closeRun for the duration of the test.
func stubCloseRun(t *testing.T, fn func(w io.Writer, opts closecmd.Options) error) {
	t.Helper()
	orig := closeRun
	closeRun = fn
	t.Cleanup(func() { closeRun = orig })
}

// stubHealthRun overrides healthRun for the duration of the test.
func stubHealthRun(t *testing.T, fn func(w io.Writer, strict bool, verbose bool) error) {
	t.Helper()
	orig := healthRun
	healthRun = fn
	t.Cleanup(func() { healthRun = orig })
}

// stubResolveActiveProfile overrides resolveActiveProfile so handler tests
// don't touch the real user's config while still letting us assert that the
// resolver is invoked (and with what argument).
func stubResolveActiveProfile(t *testing.T, fn func(explicit string) (string, error)) {
	t.Helper()
	orig := resolveActiveProfile
	resolveActiveProfile = fn
	t.Cleanup(func() { resolveActiveProfile = orig })
}

func TestRun_PRSubmit_Recognized(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubSubmitRun(t, func(io.Writer, submit.Options) error { return nil })
	err := Run([]string{"pr", "submit", "-m", "msg"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("pr submit should be recognized, got: %s", err.Error())
	}
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
}

func TestRun_PRClose_Recognized(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubCloseRun(t, func(io.Writer, closecmd.Options) error { return nil })
	err := Run([]string{"pr", "close"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("pr close should be recognized, got: %s", err.Error())
	}
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
}

func TestRun_Submit_Help_PrintsUsage(t *testing.T) {
	for _, flag := range []string{"-h", "--help"} {
		t.Run(flag, func(t *testing.T) {
			old := os.Stdout
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			os.Stdout = w

			runErr := Run([]string{"pr", "submit", flag})

			w.Close()
			os.Stdout = old

			if runErr != nil {
				t.Fatalf("expected no error for submit %s, got: %s", flag, runErr.Error())
			}
			var buf bytes.Buffer
			io.Copy(&buf, r)
			output := buf.String()
			if !strings.Contains(output, "usage: debi pr submit") {
				t.Fatalf("expected submit usage on stdout, got: %q", output)
			}
		})
	}
}

func TestRun_Close_Help_PrintsUsage(t *testing.T) {
	for _, flag := range []string{"-h", "--help"} {
		t.Run(flag, func(t *testing.T) {
			old := os.Stdout
			r, w, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			os.Stdout = w

			runErr := Run([]string{"pr", "close", flag})

			w.Close()
			os.Stdout = old

			if runErr != nil {
				t.Fatalf("expected no error for close %s, got: %s", flag, runErr.Error())
			}
			var buf bytes.Buffer
			io.Copy(&buf, r)
			output := buf.String()
			if !strings.Contains(output, "usage: debi pr close") {
				t.Fatalf("expected close usage on stdout, got: %q", output)
			}
		})
	}
}

func TestRun_PR_Help_MentionsSubmitAndClose(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"pr", "-h"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error, got: %s", runErr.Error())
	}
	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "submit") {
		t.Fatalf("pr -h should mention submit, got: %q", output)
	}
	if !strings.Contains(output, "close") {
		t.Fatalf("pr -h should mention close, got: %q", output)
	}
}

func TestRun_Submit_UnknownFlag_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "submit", "--foo"})
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

func TestRun_Submit_MissingMessage_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "submit"})
	if err == nil {
		t.Fatal("expected error for submit without --message")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "--message") {
		t.Fatalf("expected error to mention --message, got: %s", err.Error())
	}
}

func TestRun_Submit_MessageEqualsSyntax(t *testing.T) {
	captured := ""
	stubSubmitRun(t, func(w io.Writer, opts submit.Options) error {
		captured = opts.Message
		return nil
	})
	err := Run([]string{"pr", "submit", "--message=hello world"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "hello world" {
		t.Fatalf("expected captured message 'hello world', got: %q", captured)
	}
}

func TestRun_Submit_MessageSpaceSyntax(t *testing.T) {
	captured := ""
	stubSubmitRun(t, func(w io.Writer, opts submit.Options) error {
		captured = opts.Message
		return nil
	})
	err := Run([]string{"pr", "submit", "--message", "hello world"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "hello world" {
		t.Fatalf("expected captured message 'hello world', got: %q", captured)
	}
}

func TestRun_Submit_AllFlagsParsed(t *testing.T) {
	var captured submit.Options
	stubSubmitRun(t, func(w io.Writer, opts submit.Options) error {
		captured = opts
		return nil
	})
	err := Run([]string{
		"pr", "submit",
		"-m", "title",
		"-d", "body",
		"--draft",
		"-b",
		"-o",
		"--skip-tracker",
		"--json",
	})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured.Message != "title" {
		t.Fatalf("expected Message=title, got %q", captured.Message)
	}
	if captured.Description != "body" {
		t.Fatalf("expected Description=body, got %q", captured.Description)
	}
	if !captured.Draft {
		t.Fatal("expected Draft=true")
	}
	if !captured.Blocked {
		t.Fatal("expected Blocked=true")
	}
	if !captured.OpenBrowser {
		t.Fatal("expected OpenBrowser=true")
	}
	if !captured.SkipTracker {
		t.Fatal("expected SkipTracker=true")
	}
	if !captured.JSONOutput {
		t.Fatal("expected JSONOutput=true")
	}
}

func TestRun_Submit_AutoMergeFlagParsed(t *testing.T) {
	var captured submit.Options
	stubSubmitRun(t, func(w io.Writer, opts submit.Options) error {
		captured = opts
		return nil
	})
	err := Run([]string{"pr", "submit", "-m", "msg", "--auto-merge"})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if !captured.ForceAutoMerge {
		t.Fatal("expected ForceAutoMerge=true")
	}
	if captured.Blocked {
		t.Fatal("expected Blocked=false")
	}
}

func TestRun_Submit_BlockedAndAutoMerge_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "submit", "-m", "msg", "-b", "--auto-merge"})
	if err == nil {
		t.Fatal("expected error for conflicting --blocked and --auto-merge")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "--blocked") || !strings.Contains(err.Error(), "--auto-merge") {
		t.Fatalf("expected error to mention both flags, got: %s", err.Error())
	}
}

func TestRun_Close_UnknownFlag_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "close", "--foo"})
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

func TestRun_Close_AllFlagsParsed(t *testing.T) {
	var captured closecmd.Options
	stubCloseRun(t, func(w io.Writer, opts closecmd.Options) error {
		captured = opts
		return nil
	})
	err := Run([]string{
		"pr", "close",
		"-t", "https://tracker/task/1",
		"--skip-tracker",
		"-y",
	})
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured.TaskURL != "https://tracker/task/1" {
		t.Fatalf("expected TaskURL set, got %q", captured.TaskURL)
	}
	if !captured.SkipTracker {
		t.Fatal("expected SkipTracker=true")
	}
	if !captured.Force {
		t.Fatal("expected Force=true")
	}
}

func TestRun_Submit_ErrNotDetached_ReturnsUsageError(t *testing.T) {
	stubSubmitRun(t, func(io.Writer, submit.Options) error {
		return fmt.Errorf("%w (currently on branch \"foo\")", submit.ErrNotDetached)
	})
	err := Run([]string{"pr", "submit", "-m", "msg"})
	if err == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "detached HEAD") {
		t.Fatalf("expected message to mention detached HEAD, got: %s", err.Error())
	}
}

func TestRun_Submit_NotFoundError_PrintsHintAndReturnsEmptyUsageError(t *testing.T) {
	stubSubmitRun(t, func(io.Writer, submit.Options) error {
		return &credentials.NotFoundError{Provider: "asana", Service: "devora-asana", Account: "alice"}
	})

	oldStderr := os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() { os.Stderr = oldStderr }()

	runErr := Run([]string{"pr", "submit", "-m", "msg"})

	w.Close()
	os.Stderr = oldStderr

	var buf bytes.Buffer
	io.Copy(&buf, r)
	stderr := buf.String()

	if runErr == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if !errors.As(runErr, &usageErr) {
		t.Fatalf("expected UsageError (empty message), got %T: %s", runErr, runErr.Error())
	}
	if usageErr.Message != "" {
		t.Fatalf("expected empty UsageError message (stderr already printed), got: %q", usageErr.Message)
	}
	if !strings.Contains(stderr, "asana") {
		t.Fatalf("expected stderr to mention provider 'asana', got: %q", stderr)
	}
	if !strings.Contains(stderr, "security add-generic-password") && !strings.Contains(stderr, "secret-tool") && !strings.Contains(stderr, "cmdkey") && !strings.Contains(stderr, "OS keychain") {
		t.Fatalf("expected stderr to include a setup hint, got: %q", stderr)
	}
}

func TestRun_Close_ErrDetached_ReturnsUsageError(t *testing.T) {
	stubCloseRun(t, func(io.Writer, closecmd.Options) error { return closecmd.ErrDetached })
	err := Run([]string{"pr", "close"})
	if err == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "detached HEAD") {
		t.Fatalf("expected message to mention detached HEAD, got: %s", err.Error())
	}
	if !strings.Contains(err.Error(), "checkout a feature branch first") {
		t.Fatalf("expected message to include actionable suffix, got: %s", err.Error())
	}
}

func TestRun_Close_ErrProtectedBranch_ReturnsUsageError(t *testing.T) {
	stubCloseRun(t, func(io.Writer, closecmd.Options) error {
		return fmt.Errorf("%w: %q", closecmd.ErrProtectedBranch, "main")
	})
	err := Run([]string{"pr", "close"})
	if err == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "protected branch") {
		t.Fatalf("expected message to mention protected branch, got: %s", err.Error())
	}
}

func TestRun_Close_ErrNoTrackerForURL_ReturnsUsageError(t *testing.T) {
	stubCloseRun(t, func(io.Writer, closecmd.Options) error { return closecmd.ErrNoTrackerForURL })
	err := Run([]string{"pr", "close", "-t", "https://x"})
	if err == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "task-tracker") {
		t.Fatalf("expected message to mention task-tracker, got: %s", err.Error())
	}
}

func TestRun_Close_ErrAborted_ReturnsPassthroughError1(t *testing.T) {
	stubCloseRun(t, func(io.Writer, closecmd.Options) error { return closecmd.ErrAborted })
	err := Run([]string{"pr", "close"})
	if err == nil {
		t.Fatal("expected error")
	}
	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", err, err.Error())
	}
	if ptErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", ptErr.Code)
	}
}

func TestRun_Close_NotFoundError_PrintsHintAndReturnsEmptyUsageError(t *testing.T) {
	stubCloseRun(t, func(io.Writer, closecmd.Options) error {
		return &credentials.NotFoundError{Provider: "asana", Service: "devora-asana", Account: "alice"}
	})

	oldStderr := os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stderr = w
	defer func() { os.Stderr = oldStderr }()

	runErr := Run([]string{"pr", "close"})

	w.Close()
	os.Stderr = oldStderr

	var buf bytes.Buffer
	io.Copy(&buf, r)
	stderr := buf.String()

	if runErr == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if !errors.As(runErr, &usageErr) {
		t.Fatalf("expected UsageError (empty message), got %T: %s", runErr, runErr.Error())
	}
	if usageErr.Message != "" {
		t.Fatalf("expected empty UsageError message, got: %q", usageErr.Message)
	}
	if !strings.Contains(stderr, "asana") {
		t.Fatalf("expected stderr to mention provider 'asana', got: %q", stderr)
	}
}

func TestRun_Submit_OtherError_BubblesUp(t *testing.T) {
	stubSubmitRun(t, func(io.Writer, submit.Options) error { return errors.New("something weird") })
	err := Run([]string{"pr", "submit", "-m", "msg"})
	if err == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if errors.As(err, &usageErr) {
		t.Fatalf("did not expect UsageError for generic error, got: %s", err.Error())
	}
	if err.Error() != "something weird" {
		t.Fatalf("expected original error to bubble, got: %s", err.Error())
	}
}

func TestRun_Close_OtherError_BubblesUp(t *testing.T) {
	stubCloseRun(t, func(io.Writer, closecmd.Options) error { return errors.New("network down") })
	err := Run([]string{"pr", "close"})
	if err == nil {
		t.Fatal("expected error")
	}
	var usageErr *UsageError
	if errors.As(err, &usageErr) {
		t.Fatalf("did not expect UsageError for generic error, got: %s", err.Error())
	}
	if err.Error() != "network down" {
		t.Fatalf("expected original error to bubble, got: %s", err.Error())
	}
}

func TestRun_Submit_VerboseQuietCombined_ReturnsUsageError(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubSubmitRun(t, func(io.Writer, submit.Options) error { return nil })
	err := Run([]string{"pr", "submit", "-m", "msg", "-v", "-q"})
	if err == nil {
		t.Fatal("expected error when -v and -q are combined")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got: %T", err)
	}
	if !strings.Contains(err.Error(), "cannot use both") {
		t.Fatalf("expected usage message about combining flags, got: %s", err.Error())
	}
}

func TestRun_Close_VerboseQuietCombined_ReturnsUsageError(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubCloseRun(t, func(io.Writer, closecmd.Options) error { return nil })
	err := Run([]string{"pr", "close", "--verbose", "--quiet"})
	if err == nil {
		t.Fatal("expected error when -v and -q are combined")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got: %T", err)
	}
	if !strings.Contains(err.Error(), "cannot use both") {
		t.Fatalf("expected usage message about combining flags, got: %s", err.Error())
	}
}

func TestRun_Submit_VerboseFlag_SetsOption(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	var gotOpts submit.Options
	stubSubmitRun(t, func(_ io.Writer, opts submit.Options) error {
		gotOpts = opts
		return nil
	})
	if err := Run([]string{"pr", "submit", "-m", "msg", "-v"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !gotOpts.Verbose || gotOpts.Quiet {
		t.Fatalf("expected Verbose=true Quiet=false, got %+v", gotOpts)
	}
}

func TestRun_Submit_QuietFlag_SetsOption(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	var gotOpts submit.Options
	stubSubmitRun(t, func(_ io.Writer, opts submit.Options) error {
		gotOpts = opts
		return nil
	})
	if err := Run([]string{"pr", "submit", "-m", "msg", "--quiet"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotOpts.Verbose || !gotOpts.Quiet {
		t.Fatalf("expected Verbose=false Quiet=true, got %+v", gotOpts)
	}
}

func TestRun_Close_QuietFlag_SetsOption(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	var gotOpts closecmd.Options
	stubCloseRun(t, func(_ io.Writer, opts closecmd.Options) error {
		gotOpts = opts
		return nil
	})
	if err := Run([]string{"pr", "close", "-q"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !gotOpts.Quiet {
		t.Fatalf("expected Quiet=true, got %+v", gotOpts)
	}
}

// chdirToNonGit changes the test's CWD to a fresh temp directory so
// git-repo precondition checks fail.
func chdirToNonGit(t *testing.T) {
	t.Helper()
	origDir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(origDir) })
	os.Chdir(t.TempDir())
}

func TestRun_PRSubmit_OutsideGitRepo_ReturnsUsageError(t *testing.T) {
	chdirToNonGit(t)
	stubResolveActiveProfile(t, func(string) (string, error) {
		t.Fatal("resolveActiveProfile should not be called when outside a git repo")
		return "", nil
	})
	stubSubmitRun(t, func(io.Writer, submit.Options) error {
		t.Fatal("submitRun should not be called when outside a git repo")
		return nil
	})

	err := Run([]string{"pr", "submit", "-m", "msg"})
	if err == nil {
		t.Fatal("expected error when outside a git repo")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got %T: %s", err, err.Error())
	}
	if usageErr.Message != git.NotInRepoMessage {
		t.Fatalf("expected message %q, got %q", git.NotInRepoMessage, usageErr.Message)
	}
}

func TestRun_PRClose_OutsideGitRepo_ReturnsUsageError(t *testing.T) {
	chdirToNonGit(t)
	stubResolveActiveProfile(t, func(string) (string, error) {
		t.Fatal("resolveActiveProfile should not be called when outside a git repo")
		return "", nil
	})
	stubCloseRun(t, func(io.Writer, closecmd.Options) error {
		t.Fatal("closeRun should not be called when outside a git repo")
		return nil
	})

	err := Run([]string{"pr", "close"})
	if err == nil {
		t.Fatal("expected error when outside a git repo")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got %T: %s", err, err.Error())
	}
	if usageErr.Message != git.NotInRepoMessage {
		t.Fatalf("expected message %q, got %q", git.NotInRepoMessage, usageErr.Message)
	}
}

func TestRun_PRCheck_OutsideGitRepo_ReturnsUsageError(t *testing.T) {
	chdirToNonGit(t)

	err := Run([]string{"pr", "check"})
	if err == nil {
		t.Fatal("expected error when outside a git repo")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got %T: %s", err, err.Error())
	}
	if usageErr.Message != git.NotInRepoMessage {
		t.Fatalf("expected message %q, got %q", git.NotInRepoMessage, usageErr.Message)
	}
}

// --- wsgit dispatch tests ---

// stubWsgitEnsureAtWorkspaceRoot overrides wsgitEnsureAtWorkspaceRoot for the
// duration of the test.
func stubWsgitEnsureAtWorkspaceRoot(t *testing.T, fn func(cwd string) (string, error)) {
	t.Helper()
	orig := wsgitEnsureAtWorkspaceRoot
	wsgitEnsureAtWorkspaceRoot = fn
	t.Cleanup(func() { wsgitEnsureAtWorkspaceRoot = orig })
}

func stubWsgitRunStatus(t *testing.T, fn func(io.Writer, string) error) {
	t.Helper()
	orig := wsgitRunStatus
	wsgitRunStatus = fn
	t.Cleanup(func() { wsgitRunStatus = orig })
}

func stubWsgitRunClean(t *testing.T, fn func(io.Writer, string) error) {
	t.Helper()
	orig := wsgitRunClean
	wsgitRunClean = fn
	t.Cleanup(func() { wsgitRunClean = orig })
}

func stubGitGst(t *testing.T, fn func(args []string) error) {
	t.Helper()
	orig := gitGst
	gitGst = fn
	t.Cleanup(func() { gitGst = orig })
}

func stubGitGcl(t *testing.T, fn func() error) {
	t.Helper()
	orig := gitGcl
	gitGcl = fn
	t.Cleanup(func() { gitGcl = orig })
}

func TestRun_Gst_NotAtWorkspaceRoot_FallsThroughToPerRepo(t *testing.T) {
	stubWsgitEnsureAtWorkspaceRoot(t, func(string) (string, error) {
		return "", wsgit.ErrNotAtWorkspaceRoot
	})
	wsgitCalled := false
	stubWsgitRunStatus(t, func(io.Writer, string) error {
		wsgitCalled = true
		return nil
	})
	gstCalled := false
	gotArgs := []string(nil)
	stubGitGst(t, func(args []string) error {
		gstCalled = true
		gotArgs = args
		return nil
	})

	if err := Run([]string{"gst", "--short"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if wsgitCalled {
		t.Fatal("wsgitRunStatus should not be called when not at workspace root")
	}
	if !gstCalled {
		t.Fatal("git.Gst should be called when not at workspace root")
	}
	if len(gotArgs) != 1 || gotArgs[0] != "--short" {
		t.Fatalf("expected args [--short], got %v", gotArgs)
	}
}

func TestRun_Gst_AtWorkspaceRoot_RoutesToWsgit(t *testing.T) {
	stubWsgitEnsureAtWorkspaceRoot(t, func(cwd string) (string, error) {
		return "/fake/ws-1", nil
	})
	gotPath := ""
	stubWsgitRunStatus(t, func(_ io.Writer, wsPath string) error {
		gotPath = wsPath
		return nil
	})
	stubGitGst(t, func([]string) error {
		t.Fatal("git.Gst should not be called at workspace root")
		return nil
	})

	if err := Run([]string{"gst"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotPath != "/fake/ws-1" {
		t.Fatalf("expected wsPath '/fake/ws-1', got %q", gotPath)
	}
}

func TestRun_Gst_AtWorkspaceRoot_ExtraArgs_ReturnsUsageError(t *testing.T) {
	stubWsgitEnsureAtWorkspaceRoot(t, func(cwd string) (string, error) {
		return "/fake/ws-1", nil
	})
	stubWsgitRunStatus(t, func(io.Writer, string) error {
		t.Fatal("wsgitRunStatus should not be called when args are present")
		return nil
	})
	stubGitGst(t, func([]string) error {
		t.Fatal("git.Gst should not be called at workspace root")
		return nil
	})

	err := Run([]string{"gst", "--short"})
	if err == nil {
		t.Fatal("expected error when extra args at workspace root")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected *UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(usageErr.Message, "no arguments") {
		t.Fatalf("expected message to mention 'no arguments', got %q", usageErr.Message)
	}
}

func TestRun_Gcl_NotAtWorkspaceRoot_FallsThroughToPerRepo(t *testing.T) {
	stubWsgitEnsureAtWorkspaceRoot(t, func(string) (string, error) {
		return "", wsgit.ErrNotAtWorkspaceRoot
	})
	stubWsgitRunClean(t, func(io.Writer, string) error {
		t.Fatal("wsgitRunClean should not be called when not at workspace root")
		return nil
	})
	gclCalled := false
	stubGitGcl(t, func() error {
		gclCalled = true
		return nil
	})

	if err := Run([]string{"gcl"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !gclCalled {
		t.Fatal("git.Gcl should be called when not at workspace root")
	}
}

func TestRun_Gcl_AtWorkspaceRoot_RoutesToWsgit(t *testing.T) {
	stubWsgitEnsureAtWorkspaceRoot(t, func(string) (string, error) {
		return "/fake/ws-1", nil
	})
	gotPath := ""
	stubWsgitRunClean(t, func(_ io.Writer, wsPath string) error {
		gotPath = wsPath
		return nil
	})
	stubGitGcl(t, func() error {
		t.Fatal("git.Gcl should not be called at workspace root")
		return nil
	})

	if err := Run([]string{"gcl"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotPath != "/fake/ws-1" {
		t.Fatalf("expected wsPath '/fake/ws-1', got %q", gotPath)
	}
}
