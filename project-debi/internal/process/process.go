package process

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
)

type VerboseExecError struct {
	Command []string
	Err     error
	Stdout  string
	Stderr  string
}

func (e *VerboseExecError) Error() string {
	parts := []string{fmt.Sprintf("command %v failed: %v", e.Command, e.Err)}
	if s := strings.TrimSpace(e.Stdout); s != "" {
		parts = append(parts, "stdout:\n"+s)
	}
	if s := strings.TrimSpace(e.Stderr); s != "" {
		parts = append(parts, "stderr:\n"+s)
	}
	return strings.Join(parts, "\n")
}

func (e *VerboseExecError) Unwrap() error {
	return e.Err
}

type execConfig struct {
	cwd    string
	silent bool
	env    []string
	ctx    context.Context
}

type ExecOption func(*execConfig)

func WithCwd(cwd string) ExecOption {
	return func(cfg *execConfig) {
		cfg.cwd = cwd
	}
}

// WithSilent routes RunPassthrough's stdout and stderr to io.Discard. Has no
// effect on GetOutput/GetShellOutput (which always capture). stdin is left
// unchanged.
func WithSilent() ExecOption {
	return func(cfg *execConfig) {
		cfg.silent = true
	}
}

// WithExtraEnv appends key=value entries to the parent process's environment
// for the child. Repeated keys take the last value (matches exec.Command
// semantics).
func WithExtraEnv(entries ...string) ExecOption {
	return func(cfg *execConfig) {
		cfg.env = append(cfg.env, entries...)
	}
}

// WithContext binds the subprocess to ctx so cancellation/deadline kills the
// child. When the context expires, the returned error wraps ctx.Err().
func WithContext(ctx context.Context) ExecOption {
	return func(cfg *execConfig) {
		cfg.ctx = ctx
	}
}

type PassthroughError struct {
	Code int
}

func (e *PassthroughError) Error() string {
	return fmt.Sprintf("exit code %d", e.Code)
}

func RunPassthrough(command []string, opts ...ExecOption) error {
	cfg := &execConfig{}
	for _, opt := range opts {
		opt(cfg)
	}

	cmd := newCmd(cfg, command)
	if cfg.cwd != "" {
		cmd.Dir = cfg.cwd
	}
	if cfg.env != nil {
		cmd.Env = append(os.Environ(), cfg.env...)
	}

	cmd.Stdin = os.Stdin
	if cfg.silent {
		cmd.Stdout = io.Discard
		cmd.Stderr = io.Discard
	} else {
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	}

	err := cmd.Run()
	if err != nil {
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) {
			return &PassthroughError{Code: exitErr.ExitCode()}
		}
		return fmt.Errorf("failed to run %v: %w", command, err)
	}
	return nil
}

func newCmd(cfg *execConfig, command []string) *exec.Cmd {
	if cfg.ctx != nil {
		return exec.CommandContext(cfg.ctx, command[0], command[1:]...)
	}
	return exec.Command(command[0], command[1:]...)
}

func GetOutput(command []string, opts ...ExecOption) (string, error) {
	out, err := getOutputRaw(command, opts...)
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(out), nil
}

// GetOutputRaw is identical to GetOutput but does not trim whitespace from
// stdout. Use it for byte-sensitive output like `git status --porcelain -z`,
// where leading spaces in each entry encode meaningful status information.
func GetOutputRaw(command []string, opts ...ExecOption) (string, error) {
	return getOutputRaw(command, opts...)
}

func getOutputRaw(command []string, opts ...ExecOption) (string, error) {
	cfg := &execConfig{}
	for _, opt := range opts {
		opt(cfg)
	}

	cmd := newCmd(cfg, command)
	if cfg.cwd != "" {
		cmd.Dir = cfg.cwd
	}
	if cfg.env != nil {
		cmd.Env = append(os.Environ(), cfg.env...)
	}

	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err := cmd.Run()
	if err != nil {
		return "", &VerboseExecError{
			Command: command,
			Err:     err,
			Stdout:  strings.TrimSpace(stdout.String()),
			Stderr:  strings.TrimSpace(stderr.String()),
		}
	}
	return stdout.String(), nil
}

func GetShellOutput(command string, opts ...ExecOption) (string, error) {
	return GetOutput([]string{"sh", "-c", command}, opts...)
}
