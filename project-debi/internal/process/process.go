package process

import (
	"bytes"
	"errors"
	"fmt"
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
	cwd string
}

type ExecOption func(*execConfig)

func WithCwd(cwd string) ExecOption {
	return func(cfg *execConfig) {
		cfg.cwd = cwd
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

	cmd := exec.Command(command[0], command[1:]...)
	if cfg.cwd != "" {
		cmd.Dir = cfg.cwd
	}

	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

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

func GetOutput(command []string, opts ...ExecOption) (string, error) {
	cfg := &execConfig{}
	for _, opt := range opts {
		opt(cfg)
	}

	cmd := exec.Command(command[0], command[1:]...)
	if cfg.cwd != "" {
		cmd.Dir = cfg.cwd
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
	return strings.TrimSpace(stdout.String()), nil
}

func GetShellOutput(command string, opts ...ExecOption) (string, error) {
	return GetOutput([]string{"sh", "-c", command}, opts...)
}
