package crash

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime/debug"
	"time"
)

var crashDir = os.TempDir()
var stderrWriter io.Writer = os.Stderr

func HandleError(err error) {
	crashFile := writeCrashLog(err.Error())
	if crashFile == "" {
		fmt.Fprintf(stderrWriter, "Devora crashed unexpectedly: %v\n", err)
		return
	}
	fmt.Fprintf(stderrWriter, "Devora crashed unexpectedly. Details written to %s\n", crashFile)
}

func HandlePanic(recovered any) {
	stack := debug.Stack()
	content := fmt.Sprintf("panic: %v\n\n%s", recovered, stack)
	crashFile := writeCrashLog(content)
	if crashFile == "" {
		fmt.Fprintf(stderrWriter, "Devora crashed unexpectedly: %v\n%s\n", recovered, stack)
		return
	}
	fmt.Fprintf(stderrWriter, "Devora crashed unexpectedly. Details written to %s\n", crashFile)
}

func writeCrashLog(content string) string {
	timestamp := time.Now().Format("20060102_150405")
	crashFile := filepath.Join(crashDir, fmt.Sprintf("devora_crash_%s.log", timestamp))
	if err := os.WriteFile(crashFile, []byte(content), 0644); err != nil {
		return ""
	}
	return crashFile
}
