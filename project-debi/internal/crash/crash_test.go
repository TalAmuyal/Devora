package crash

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestHandleError_WritesFile(t *testing.T) {
	dir := t.TempDir()
	origDir := crashDir
	crashDir = dir
	t.Cleanup(func() { crashDir = origDir })

	var stderrBuf bytes.Buffer
	origStderr := stderrWriter
	stderrWriter = &stderrBuf
	t.Cleanup(func() { stderrWriter = origStderr })

	HandleError(errors.New("something broke"))

	files, err := filepath.Glob(filepath.Join(dir, "devora_crash_*.log"))
	if err != nil {
		t.Fatalf("glob error: %v", err)
	}
	if len(files) != 1 {
		t.Fatalf("expected 1 crash file, got %d", len(files))
	}

	content, err := os.ReadFile(files[0])
	if err != nil {
		t.Fatalf("read crash file: %v", err)
	}
	if string(content) != "something broke" {
		t.Fatalf("expected crash file content %q, got %q", "something broke", string(content))
	}

	stderrOut := stderrBuf.String()
	if !strings.Contains(stderrOut, "Devora crashed unexpectedly") {
		t.Fatalf("expected stderr to contain crash message, got: %s", stderrOut)
	}
	if !strings.Contains(stderrOut, files[0]) {
		t.Fatalf("expected stderr to contain crash file path, got: %s", stderrOut)
	}
}

func TestHandleError_CrashFileNameFormat(t *testing.T) {
	dir := t.TempDir()
	origDir := crashDir
	crashDir = dir
	t.Cleanup(func() { crashDir = origDir })

	origStderr := stderrWriter
	stderrWriter = &bytes.Buffer{}
	t.Cleanup(func() { stderrWriter = origStderr })

	HandleError(errors.New("test"))

	files, _ := filepath.Glob(filepath.Join(dir, "devora_crash_*.log"))
	if len(files) != 1 {
		t.Fatalf("expected 1 crash file, got %d", len(files))
	}
	base := filepath.Base(files[0])
	if !strings.HasPrefix(base, "devora_crash_") || !strings.HasSuffix(base, ".log") {
		t.Fatalf("unexpected crash file name: %s", base)
	}
}

func TestHandlePanic_IncludesStackTrace(t *testing.T) {
	dir := t.TempDir()
	origDir := crashDir
	crashDir = dir
	t.Cleanup(func() { crashDir = origDir })

	origStderr := stderrWriter
	stderrWriter = &bytes.Buffer{}
	t.Cleanup(func() { stderrWriter = origStderr })

	HandlePanic("oh no")

	files, _ := filepath.Glob(filepath.Join(dir, "devora_crash_*.log"))
	if len(files) != 1 {
		t.Fatalf("expected 1 crash file, got %d", len(files))
	}

	content, err := os.ReadFile(files[0])
	if err != nil {
		t.Fatalf("read crash file: %v", err)
	}

	contentStr := string(content)
	if !strings.Contains(contentStr, "oh no") {
		t.Fatalf("expected crash file to contain panic value, got: %s", contentStr)
	}
	if !strings.Contains(contentStr, "goroutine") {
		t.Fatalf("expected crash file to contain stack trace, got: %s", contentStr)
	}
}
