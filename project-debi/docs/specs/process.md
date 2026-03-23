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

## Functions

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

## Notes

- No timeout support is needed at this time. Commands are expected to complete in reasonable time.
- Shell commands (single string) and argument-list commands are separated into two distinct functions (`GetShellOutput` and `GetOutput`) for clarity.
