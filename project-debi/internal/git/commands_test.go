package git_test

import (
	"bytes"
	"errors"
	"io"
	"os"
	"strings"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
)

func TestGri_InvalidArgument_ReturnsPassthroughError(t *testing.T) {
	err := git.Gri([]string{"abc"})
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

func TestGri_ZeroArgument_ReturnsPassthroughError(t *testing.T) {
	err := git.Gri([]string{"0"})
	if err == nil {
		t.Fatal("expected error for zero argument")
	}

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
}

func TestGri_NegativeArgument_ReturnsPassthroughError(t *testing.T) {
	err := git.Gri([]string{"-1"})
	if err == nil {
		t.Fatal("expected error for negative argument")
	}

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
}

func TestGri_HelpFlag_PrintsUsageAndReturnsNil(t *testing.T) {
	// Capture stdout
	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	err := git.Gri([]string{"--help"})

	w.Close()
	os.Stdout = oldStdout

	if err != nil {
		t.Fatalf("expected nil error for --help, got: %v", err)
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Usage:") {
		t.Fatalf("expected help output to contain 'Usage:', got: %s", output)
	}
}

func TestGri_ShortHelpFlag_PrintsUsageAndReturnsNil(t *testing.T) {
	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	err := git.Gri([]string{"-h"})

	w.Close()
	os.Stdout = oldStdout

	if err != nil {
		t.Fatalf("expected nil error for -h, got: %v", err)
	}

	var buf bytes.Buffer
	io.Copy(&buf, r)
	output := buf.String()
	if !strings.Contains(output, "Usage:") {
		t.Fatalf("expected help output to contain 'Usage:', got: %s", output)
	}
}

