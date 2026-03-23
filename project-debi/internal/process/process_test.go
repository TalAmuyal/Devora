package process

import (
	"errors"
	"os/exec"
	"path/filepath"
	"testing"
)

func TestGetOutput_Success(t *testing.T) {
	out, err := GetOutput([]string{"echo", "hello"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if out != "hello" {
		t.Fatalf("expected %q, got %q", "hello", out)
	}
}

func TestGetOutput_TrimsWhitespace(t *testing.T) {
	out, err := GetOutput([]string{"echo", "  padded  "})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if out != "padded" {
		t.Fatalf("expected %q, got %q", "padded", out)
	}
}

func TestGetOutput_FailedCommand(t *testing.T) {
	_, err := GetOutput([]string{"false"})
	if err == nil {
		t.Fatal("expected error for failed command")
	}

	var execErr *VerboseExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected *VerboseExecError, got %T", err)
	}
}

func TestGetOutput_VerboseExecError_IncludesStderr(t *testing.T) {
	_, err := GetOutput([]string{"sh", "-c", "echo oops >&2; exit 1"})
	if err == nil {
		t.Fatal("expected error")
	}

	var execErr *VerboseExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected *VerboseExecError, got %T", err)
	}

	if execErr.Stderr != "oops" {
		t.Fatalf("expected stderr %q, got %q", "oops", execErr.Stderr)
	}

	errStr := execErr.Error()
	if !contains(errStr, "stderr:") {
		t.Fatalf("error string should contain stderr section, got: %s", errStr)
	}
	if !contains(errStr, "oops") {
		t.Fatalf("error string should contain stderr content, got: %s", errStr)
	}
}

func TestGetOutput_VerboseExecError_OmitsEmptySections(t *testing.T) {
	_, err := GetOutput([]string{"false"})
	if err == nil {
		t.Fatal("expected error")
	}

	errStr := err.Error()
	if contains(errStr, "stdout:") {
		t.Fatalf("error string should not contain stdout section for empty stdout, got: %s", errStr)
	}
	if contains(errStr, "stderr:") {
		t.Fatalf("error string should not contain stderr section for empty stderr, got: %s", errStr)
	}
}

func TestGetOutput_VerboseExecError_Unwrap(t *testing.T) {
	_, err := GetOutput([]string{"false"})
	if err == nil {
		t.Fatal("expected error")
	}

	var execErr *VerboseExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected *VerboseExecError, got %T", err)
	}

	unwrapped := execErr.Unwrap()
	var exitErr *exec.ExitError
	if !errors.As(unwrapped, &exitErr) {
		t.Fatalf("expected *exec.ExitError from Unwrap, got %T", unwrapped)
	}
}

func TestGetOutput_WithCwd(t *testing.T) {
	dir := t.TempDir()
	out, err := GetOutput([]string{"pwd"}, WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Resolve symlinks for comparison (macOS /tmp -> /private/tmp)
	expectedDir, _ := filepath.EvalSymlinks(dir)
	actualDir, _ := filepath.EvalSymlinks(out)
	if actualDir != expectedDir {
		t.Fatalf("expected cwd %q, got %q", expectedDir, actualDir)
	}
}

func TestGetShellOutput_Success(t *testing.T) {
	out, err := GetShellOutput("echo hello world")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if out != "hello world" {
		t.Fatalf("expected %q, got %q", "hello world", out)
	}
}

func TestGetShellOutput_ShellFeatures(t *testing.T) {
	out, err := GetShellOutput("echo foo; echo bar")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	expected := "foo\nbar"
	if out != expected {
		t.Fatalf("expected %q, got %q", expected, out)
	}
}

func TestGetShellOutput_WithCwd(t *testing.T) {
	dir := t.TempDir()
	out, err := GetShellOutput("pwd", WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedDir, _ := filepath.EvalSymlinks(dir)
	actualDir, _ := filepath.EvalSymlinks(out)
	if actualDir != expectedDir {
		t.Fatalf("expected cwd %q, got %q", expectedDir, actualDir)
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && searchString(s, substr)
}

func searchString(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
