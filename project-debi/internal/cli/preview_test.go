package cli

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// stubPreviewPost overrides previewPost for the duration of the test.
func stubPreviewPost(t *testing.T, fn func(port string, ptyID int, path string, stack bool) error) {
	t.Helper()
	orig := previewPost
	previewPost = fn
	t.Cleanup(func() { previewPost = orig })
}

// withinEmber sets the env vars Ember injects into a PTY shell.
func withinEmber(t *testing.T) {
	t.Helper()
	t.Setenv("DEVORA_IPC_PORT", "9999")
	t.Setenv("DEVORA_PTY_ID", "7")
}

func expectUsageError(t *testing.T, err error) {
	t.Helper()
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %v", err, err)
	}
}

func TestRunPreview_NoArgs_ReturnsUsageError(t *testing.T) {
	withinEmber(t)
	expectUsageError(t, runPreview([]string{}))
}

func TestRunPreview_UnknownFlag_ReturnsUsageError(t *testing.T) {
	withinEmber(t)
	expectUsageError(t, runPreview([]string{"--nope", "x.md"}))
}

func TestRunPreview_Help_PrintsUsage(t *testing.T) {
	stop := captureStdout(t)
	err := runPreview([]string{"--help"})
	out := stop()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(out, "usage: debi preview") {
		t.Fatalf("expected usage text, got %q", out)
	}
}

func TestRunPreview_OutsideEmber_ReturnsUsageError(t *testing.T) {
	t.Setenv("DEVORA_IPC_PORT", "")
	t.Setenv("DEVORA_PTY_ID", "")
	err := runPreview([]string{"x.md"})
	expectUsageError(t, err)
	if !strings.Contains(err.Error(), "inside a Devora terminal") {
		t.Fatalf("expected Devora-terminal hint, got %q", err.Error())
	}
}

func TestRunPreview_MissingFile_ReturnsUsageError(t *testing.T) {
	withinEmber(t)
	stubPreviewPost(t, func(string, int, string, bool) error {
		t.Fatal("previewPost should not be called for an invalid file")
		return nil
	})
	expectUsageError(t, runPreview([]string{filepath.Join(t.TempDir(), "missing.md")}))
}

func TestRunPreview_Directory_ReturnsUsageError(t *testing.T) {
	withinEmber(t)
	stubPreviewPost(t, func(string, int, string, bool) error {
		t.Fatal("previewPost should not be called for a directory")
		return nil
	})
	expectUsageError(t, runPreview([]string{t.TempDir()}))
}

func TestRunPreview_UnsupportedExtension_ReturnsUsageError(t *testing.T) {
	withinEmber(t)
	path := filepath.Join(t.TempDir(), "notes.txt")
	if err := os.WriteFile(path, []byte("hi"), 0o644); err != nil {
		t.Fatal(err)
	}
	stubPreviewPost(t, func(string, int, string, bool) error {
		t.Fatal("previewPost should not be called for an unsupported extension")
		return nil
	})
	expectUsageError(t, runPreview([]string{path}))
}

func TestRunPreview_TooLarge_ReturnsUsageError(t *testing.T) {
	withinEmber(t)
	path := filepath.Join(t.TempDir(), "big.md")
	if err := os.WriteFile(path, make([]byte, previewMaxFileSize+1), 0o644); err != nil {
		t.Fatal(err)
	}
	stubPreviewPost(t, func(string, int, string, bool) error {
		t.Fatal("previewPost should not be called for an oversized file")
		return nil
	})
	expectUsageError(t, runPreview([]string{path}))
}

func TestRunPreview_Stack_SendsAbsolutePathPortAndStack(t *testing.T) {
	withinEmber(t)
	path := filepath.Join(t.TempDir(), "doc.md")
	if err := os.WriteFile(path, []byte("# hi"), 0o644); err != nil {
		t.Fatal(err)
	}

	var gotPort, gotPath string
	var gotID int
	var gotStack bool
	stubPreviewPost(t, func(port string, ptyID int, p string, stack bool) error {
		gotPort, gotID, gotPath, gotStack = port, ptyID, p, stack
		return nil
	})

	if err := runPreview([]string{"--stack", path}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotPort != "9999" || gotID != 7 || !gotStack {
		t.Fatalf("unexpected args: port=%q id=%d stack=%v", gotPort, gotID, gotStack)
	}
	// macOS temp dirs are symlinks (/var -> /private/var); the command canonicalizes.
	want, _ := filepath.EvalSymlinks(path)
	if gotPath != want {
		t.Fatalf("expected canonical path %q, got %q", want, gotPath)
	}
}

func TestRunPreview_RelativePath_ResolvedAgainstCWD(t *testing.T) {
	withinEmber(t)
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "readme.md"), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	t.Chdir(dir)

	var gotPath string
	stubPreviewPost(t, func(_ string, _ int, p string, _ bool) error {
		gotPath = p
		return nil
	})
	if err := runPreview([]string{"readme.md"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	want, _ := filepath.EvalSymlinks(filepath.Join(dir, "readme.md"))
	if gotPath != want {
		t.Fatalf("expected %q, got %q", want, gotPath)
	}
}

func TestPostPreviewOpen_SendsExpectedRequest(t *testing.T) {
	var gotPath string
	var gotBody map[string]any
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotPath = r.URL.Path
		_ = json.NewDecoder(r.Body).Decode(&gotBody)
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer ts.Close()

	u, _ := url.Parse(ts.URL)
	if err := postPreviewOpen(u.Port(), 42, "/abs/x.md", true); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotPath != "/preview/open" {
		t.Fatalf("expected POST to /preview/open, got %s", gotPath)
	}
	if gotBody["ptyId"].(float64) != 42 {
		t.Fatalf("expected ptyId 42, got %v", gotBody["ptyId"])
	}
	if gotBody["path"].(string) != "/abs/x.md" {
		t.Fatalf("expected path /abs/x.md, got %v", gotBody["path"])
	}
	if gotBody["stack"].(bool) != true {
		t.Fatalf("expected stack true, got %v", gotBody["stack"])
	}
}

func TestPostPreviewOpen_NonOKStatus_ReturnsError(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer ts.Close()

	u, _ := url.Parse(ts.URL)
	if err := postPreviewOpen(u.Port(), 1, "/x.md", false); err == nil {
		t.Fatal("expected an error for a non-200 response")
	}
}
