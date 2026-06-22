package cli

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

const previewUsage = `usage: debi preview [--stack] <file>

Render a Markdown or HTML file in a preview pane to the right of the terminal.
By default the file replaces the current preview; --stack opens it in an additional pane alongside the existing one(s).
Re-previewing the same file refreshes its pane.

Only available when running inside a Devora (Ember) terminal.

Flags:
  --stack     Open in a new pane instead of replacing the current preview
  -h, --help  Show this help`

// previewMaxFileSize caps the size of a file we ask Ember to render.
// The frontend renders the whole document synchronously on the UI thread, so a very large file would freeze the window.
const previewMaxFileSize = 5 * 1024 * 1024 // 5 MiB

// previewExtensions is the set of file extensions Ember can render.
var previewExtensions = map[string]bool{
	".md":       true,
	".markdown": true,
	".html":     true,
	".htm":      true,
}

// previewPost is the IPC entry point; stubbable for tests.
var previewPost = postPreviewOpen

func runPreview(args []string) error {
	stack := false
	file := ""

	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "-h" || arg == "--help":
			fmt.Println(previewUsage)
			return nil
		case arg == "--stack":
			stack = true
		case strings.HasPrefix(arg, "-"):
			return &UsageError{Message: fmt.Sprintf("unknown flag: %s\n%s", arg, previewUsage)}
		default:
			if file != "" {
				return &UsageError{Message: fmt.Sprintf("unexpected argument: %s\n%s", arg, previewUsage)}
			}
			file = arg
		}
	}

	if file == "" {
		return &UsageError{Message: previewUsage}
	}

	// Running inside a Devora terminal is a precondition: Ember injects these when it spawns the PTY (see ipc_server.rs / pty.rs).
	port := os.Getenv("DEVORA_IPC_PORT")
	ptyID := os.Getenv("DEVORA_PTY_ID")
	if port == "" || ptyID == "" {
		return &UsageError{Message: "debi preview is only available inside a Devora terminal"}
	}
	id, err := strconv.Atoi(ptyID)
	if err != nil {
		return &UsageError{Message: fmt.Sprintf("invalid DEVORA_PTY_ID: %s", ptyID)}
	}

	path, err := resolvePreviewFile(file)
	if err != nil {
		return err
	}

	return previewPost(port, id, path, stack)
}

// resolvePreviewFile turns the user's argument into a canonical absolute path and validates that it is a previewable file.
// The frontend reads the path with a bare fs::read_to_string and has no CWD context, so it must be absolute.
func resolvePreviewFile(file string) (string, error) {
	abs, err := filepath.Abs(file)
	if err != nil {
		return "", &UsageError{Message: fmt.Sprintf("cannot resolve path: %s", file)}
	}

	info, err := os.Stat(abs)
	if err != nil {
		if os.IsNotExist(err) {
			return "", &UsageError{Message: fmt.Sprintf("no such file: %s", file)}
		}
		return "", &UsageError{Message: fmt.Sprintf("cannot read %s: %s", file, err)}
	}
	if info.IsDir() {
		return "", &UsageError{Message: fmt.Sprintf("not a file: %s", file)}
	}
	if info.Size() > previewMaxFileSize {
		return "", &UsageError{Message: fmt.Sprintf(
			"file is too large to preview (%d bytes, limit %d)", info.Size(), previewMaxFileSize)}
	}

	ext := strings.ToLower(filepath.Ext(abs))
	if !previewExtensions[ext] {
		return "", &UsageError{Message: fmt.Sprintf(
			"unsupported file type %q (expected .md, .markdown, .html, or .htm)", ext)}
	}

	// Canonicalize so "same file" comparison in Ember (refresh vs new pane) is stable across symlinks.
	if resolved, err := filepath.EvalSymlinks(abs); err == nil {
		abs = resolved
	}
	return abs, nil
}

// postPreviewOpen sends a fire-and-forget request to Ember's IPC server asking it to render path in a preview pane for the given PTY session.
func postPreviewOpen(port string, ptyID int, path string, stack bool) error {
	body, err := json.Marshal(map[string]any{
		"ptyId": ptyID,
		"path":  path,
		"stack": stack,
	})
	if err != nil {
		return err
	}

	url := fmt.Sprintf("http://127.0.0.1:%s/preview/open", port)
	resp, err := http.Post(url, "application/json", bytes.NewReader(body))
	if err != nil {
		return fmt.Errorf("failed to reach Devora: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("Devora returned status %d", resp.StatusCode)
	}
	return nil
}
