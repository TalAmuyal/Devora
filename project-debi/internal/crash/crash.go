// Package crash reports failures that escaped the CLI's error handling (see main.go).
// Expected failures, like usage errors or missing preconditions, must be handled before reaching it.
//
// Details are written to a timestamped log in os.TempDir(), which outlives the terminal, and echoed to stderr, which still reaches the user if the log can't be written.
package crash

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime/debug"
	"time"
)

// Stubbable package-level vars for tests.
var (
	crashDir               = os.TempDir()
	stderrWriter io.Writer = os.Stderr
)

func HandleError(err error) {
	content := err.Error()
	crashFile := writeCrashLog(content)
	if crashFile == "" {
		fmt.Fprintf(stderrWriter, "Devora crashed unexpectedly: %v\n", err)
		return
	}
	fmt.Fprintln(stderrWriter, "Devora crashed unexpectedly. Details written to "+crashFile)
	fmt.Fprintln(stderrWriter, "---")
	fmt.Fprintln(stderrWriter, content)
}

func HandlePanic(recovered any) {
	stack := debug.Stack()
	content := fmt.Sprintf("panic: %v\n\n%s", recovered, stack)
	crashFile := writeCrashLog(content)
	if crashFile == "" {
		fmt.Fprintf(stderrWriter, "Devora crashed unexpectedly: %v\n%s\n", recovered, stack)
		return
	}
	fmt.Fprintln(stderrWriter, "Devora crashed unexpectedly. Details written to "+crashFile)
	fmt.Fprintln(stderrWriter, "---")
	fmt.Fprintln(stderrWriter, content)
}

func writeCrashLog(content string) string {
	timestamp := time.Now().Format("20060102_150405")
	crashFile := filepath.Join(crashDir, fmt.Sprintf("devora_crash_%s.log", timestamp))
	if err := os.WriteFile(crashFile, []byte(content), 0644); err != nil {
		return ""
	}
	return crashFile
}
