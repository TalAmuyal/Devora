package cli

import (
	"errors"
	"strings"
	"testing"
)

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
	// add command attempts to resolve workspace from CWD - will fail in test env
	err := Run([]string{"add"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("add should be recognized, got: %s", err.Error())
	}
	// Expected: "not inside a known workspace" since tests don't run from a workspace
	if err != nil && !strings.Contains(err.Error(), "workspace") {
		t.Fatalf("expected workspace-related error, got: %s", err.Error())
	}
}

func TestRun_Add_Alias(t *testing.T) {
	err := Run([]string{"a"})
	if err != nil && strings.Contains(err.Error(), "unknown command") {
		t.Fatalf("a alias should be recognized, got: %s", err.Error())
	}
}
