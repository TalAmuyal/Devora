package git_test

import (
	"errors"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
)

func TestRebaseInteractive_InvalidArgument_ReturnsPassthroughError(t *testing.T) {
	err := git.RebaseInteractive([]string{"abc"})
	if err == nil {
		t.Fatal("expected error for invalid argument")
	}

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
	if ptErr.Code != 1 {
		t.Fatalf("expected exit code 1, got %d", ptErr.Code)
	}
}

func TestRebaseInteractive_ZeroArgument_ReturnsPassthroughError(t *testing.T) {
	err := git.RebaseInteractive([]string{"0"})
	if err == nil {
		t.Fatal("expected error for zero argument")
	}

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
}

func TestRebaseInteractive_NegativeArgument_ReturnsPassthroughError(t *testing.T) {
	err := git.RebaseInteractive([]string{"-1"})
	if err == nil {
		t.Fatal("expected error for negative argument")
	}

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
}

// Help flags (-h/--help) are short-circuited by gitcmd's runCustom before RebaseInteractive is reached; see gitcmd's TestRun_CustomHelp_ShortCircuits
