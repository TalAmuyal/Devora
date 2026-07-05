package gitcmd

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"
	"sync"

	"devora/internal/process"
	"devora/internal/style"
)

const fanOutConcurrency = 8 // Avoid an uncapped spawn that could open dozens of ssh connections at once

var stdoutIsTTY = func() bool {
	fi, err := os.Stdout.Stat()
	return err == nil && fi.Mode()&os.ModeCharDevice != 0
}

func fanOut(w io.Writer, args []string, repos []string) error {
	gitArgs := args
	if stdoutIsTTY() {
		// Captured output looks like a pipe to git, which would disable color even though the aggregate lands on a TTY.
		// `color.ui` is the only mechanism that covers every fan-out subcommand (--color is not valid for all of them).
		// No pager handling is needed: git never pages into a pipe.
		gitArgs = append([]string{"-c", "color.ui=always"}, args...)
	}

	// One buffer per repo, shared by stdout and stderr: os/exec serializes writes when both are the same comparable writer, so this is race-free and keeps each repo's output interleaved the way a terminal would
	outputs := make([]bytes.Buffer, len(repos))
	errs := make([]error, len(repos))
	sem := make(chan struct{}, fanOutConcurrency)
	var wg sync.WaitGroup
	for i, name := range repos {
		wg.Add(1)
		go func(i int, name string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			errs[i] = runPassthrough(
				append([]string{"git"}, gitArgs...),
				process.WithCwd(name),
				process.WithStdout(&outputs[i]),
				process.WithStderr(&outputs[i]),
				process.WithNoStdin(),
				process.WithExtraEnv(fanOutEnv()...),
			)
		}(i, name)
	}
	wg.Wait()

	exitCode := 0
	for i, name := range repos {
		fmt.Fprintln(w, style.Info.Render("## "+name))
		w.Write(outputs[i].Bytes())
		if errs[i] != nil {
			var ptErr *process.PassthroughError
			if errors.As(errs[i], &ptErr) {
				exitCode = ptErr.Code
			} else {
				// Failure to even start git (not an exit code): surface the message in this repo's section and fail the run
				fmt.Fprintln(w, errs[i].Error())
				exitCode = 1
			}
		}
		fmt.Fprintln(w)
	}
	if exitCode != 0 {
		return &process.PassthroughError{Code: exitCode}
	}
	return nil
}

func fanOutEnv() []string {
	env := []string{"GIT_TERMINAL_PROMPT=0"}
	if _, ok := os.LookupEnv("GIT_SSH_COMMAND"); !ok {
		env = append(env, "GIT_SSH_COMMAND=ssh -o BatchMode=yes")
	}
	return env
}
