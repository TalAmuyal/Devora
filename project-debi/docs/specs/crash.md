# Crash Package Spec

Package: `internal/crash`

## Purpose

Handle unexpected errors and panics by writing crash logs to a known location and printing a user-friendly message to stderr.

## Crash Log Location

Crash logs are written to the system temp directory (`os.TempDir()`, typically `/tmp` on macOS/Linux).

Filename format: `devora_crash_YYYYMMDD_HHMMSS.log`

Example: `/tmp/devora_crash_20260313_143022.log`

## Functions

### HandleError

```go
func HandleError(err error)
```

Called when the CLI returns an error. Behavior:
1. Generate a timestamp in `YYYYMMDD_HHMMSS` format.
2. Construct the crash file path.
3. Write `err.Error()` to the crash file.
4. Print to stderr:
   - `Devora crashed unexpectedly. Details written to <path>`
   - `---`
   - The full error content (same as the crash file).
5. If writing the crash file itself fails, print `Devora crashed unexpectedly: <error>` to stderr as a fallback (no separator line, no duplicate content).

### HandlePanic

```go
func HandlePanic(recovered any)
```

Called from a `recover()` in `main()`. Behavior:
1. Format the panic value and stack trace (using `runtime/debug.Stack()`).
2. Generate a timestamp and construct the crash file path (same format as `HandleError`).
3. Write the formatted panic + stack trace to the crash file.
4. Print to stderr:
   - `Devora crashed unexpectedly. Details written to <path>`
   - `---`
   - The full panic content (same as the crash file).
5. If writing fails, print `Devora crashed unexpectedly: <panic value>\n<stack trace>` to stderr (no separator line).

## Integration with main()

The crash handler is used in `main.go` (see [cli.md](cli.md#integration-with-maingo) for the full code block):

- `crash.HandlePanic(r)` is called from a `defer`/`recover` block to handle panics.
- `crash.HandleError(err)` is called for non-`UsageError` errors returned by `cli.Run`.
- `cli.UsageError` is handled separately — usage errors are printed to stderr without crash logging.

Note: Ctrl+C sends SIGINT which terminates the process. Bubble Tea handles SIGINT gracefully within the TUI.

## Testing

- Test that `HandleError` writes a crash file with the error message.
- Test that the crash file name matches the expected format.
- Test that stderr output contains the crash file path, the `---` separator line, and the full error content.
- Test `HandlePanic` includes stack trace information in the crash file and also echoes the panic value plus stack trace to stderr after the `---` separator.
- Use a temp directory override or verify files appear in `os.TempDir()`.
