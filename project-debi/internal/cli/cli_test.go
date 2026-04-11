package cli

import (
	"bytes"
	"devora/internal/process"
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

func TestRun_PRStatus_Help_PrintsUsage(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"pr", "status", "-h"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for pr status -h, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "usage: debi pr status") {
		t.Fatalf("expected pr status usage message on stdout, got: %q", output)
	}
}

func TestRun_PRStatus_UnknownFlag_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"pr", "status", "--foo"})
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

func TestRun_PRS_UnknownFlag_ReturnsUsageError(t *testing.T) {
	err := Run([]string{"prs", "--foo"})
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

func TestRun_PRS_Help_PrintsUsage(t *testing.T) {
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w

	runErr := Run([]string{"prs", "-h"})

	w.Close()
	os.Stdout = old

	if runErr != nil {
		t.Fatalf("expected no error for prs -h, got: %s", runErr.Error())
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "usage: debi pr status") {
		t.Fatalf("expected pr status usage message on stdout, got: %q", output)
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

func TestCommandIndex_AllCommandsAndAliasesResolvable(t *testing.T) {
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
