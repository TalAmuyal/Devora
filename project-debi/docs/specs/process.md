# Process Package Spec

Package: `internal/process`

## Purpose

Execute shell commands and return their output. This is the foundation package - nearly every other package depends on it for running git, kitty, and mise commands.

## Types

### VerboseExecError

```go
type VerboseExecError struct {
    Command []string
    Err     error
    Stdout  string
    Stderr  string
}
```

- `Error() string` - Includes the command, the underlying error, and stdout/stderr if non-empty. Format:
  ```
  command [git status --porcelain] failed: exit status 1
  stdout:
  <stdout content>
  stderr:
  <stderr content>
  ```
  Stdout and stderr sections are only included if non-empty (after trimming whitespace).

- `Unwrap() error` - Returns the underlying `Err` for use with `errors.Is`/`errors.As`.

### ExecOption

```go
type ExecOption func(*execConfig)
```

Functional options for configuring command execution.

#### WithCwd

```go
func WithCwd(cwd string) ExecOption
```

Sets the working directory for the command.

#### WithSilent

```go
func WithSilent() ExecOption
```

Routes `RunPassthrough`'s stdout and stderr to `io.Discard`. Has no effect on `GetOutput`/`GetShellOutput` (which always capture). `stdin` is left unchanged. Used by submit/close in Normal and Quiet modes to keep git/gh subprocess output off the user's terminal while still enforcing exit-code semantics (`*PassthroughError` is still returned on failure).

### PassthroughError

```go
type PassthroughError struct {
    Code int
}
```

- `Error() string` - Returns `"exit code <Code>"`.

Represents a command that exited with a non-zero status when run in passthrough mode. Used by `main.go` to exit with the same code without crash logging.

## Functions

### RunPassthrough

```go
func RunPassthrough(command []string, opts ...ExecOption) error
```

Executes a command with stdin/stdout/stderr connected directly to the terminal (passthrough mode). Used for git shortcuts where the user interacts with the command directly.

Behavior:
1. Create `exec.Cmd` from `command[0]` (program) and `command[1:]` (args).
2. Apply options (e.g., set `cmd.Dir` from `WithCwd`, `silent` from `WithSilent`).
3. Connect `cmd.Stdin = os.Stdin`. Connect `cmd.Stdout`/`cmd.Stderr` to `os.Stdout`/`os.Stderr` by default; route both to `io.Discard` when `WithSilent` is applied.
4. Run the command.
5. If the command fails with `*exec.ExitError`, return `&PassthroughError{Code: exitErr.ExitCode()}`.
6. If the command fails with another error, return `fmt.Errorf("failed to run %v: %w", command, err)`.
7. On success, return nil.

### GetOutput

```go
func GetOutput(command []string, opts ...ExecOption) (string, error)
```

Executes a command with explicit arguments (no shell interpretation). Returns the trimmed stdout on success.

Behavior:
1. Create `exec.Cmd` from `command[0]` (program) and `command[1:]` (args).
2. Apply options (e.g., set `cmd.Dir` from `WithCwd`).
3. Capture stdout and stderr into buffers.
4. Run the command.
5. If the command fails (non-zero exit), return a `*VerboseExecError` with trimmed stdout and stderr.
6. On success, return `strings.TrimSpace(stdout)`.

### GetShellOutput

```go
func GetShellOutput(command string, opts ...ExecOption) (string, error)
```

Executes a shell command string (with shell interpretation). Equivalent to `GetOutput([]string{"sh", "-c", command}, opts...)`.

This is needed for commands that use shell features (pipes, redirections) or are passed as single strings from config/other systems. Used by the Kitty backend for some commands.

## Testing

- Test `GetOutput` with simple commands (`echo hello`, `true`, `false`).
- Test that `VerboseExecError` includes stdout/stderr from failed commands.
- Test that `VerboseExecError` omits empty stdout/stderr sections.
- Test `WithCwd` by running a command that outputs the working directory.
- Test `GetShellOutput` with a shell expression.
- Test `Unwrap` returns the underlying error.
- Test `RunPassthrough` with a successful command (`true`) returns nil.
- Test `RunPassthrough` with a failing command (`false`) returns `*PassthroughError` with correct exit code.
- Test `PassthroughError.Error()` returns the expected message format.
- Test `RunPassthrough` with `WithCwd` sets the working directory correctly.
- Test `RunPassthrough` with `WithSilent` runs cleanly (no terminal output; exit code still propagates as `*PassthroughError`).

## Notes

- No timeout support is needed at this time. Commands are expected to complete in reasonable time.
- Shell commands (single string) and argument-list commands are separated into two distinct functions (`GetShellOutput` and `GetOutput`) for clarity.
