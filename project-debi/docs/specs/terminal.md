# Terminal Package Spec

Package: `internal/terminal`

The terminal package abstracts over terminal session management via Kitty's remote control protocol. It provides a `Backend` interface with a `KittyBackend` implementation and high-level session management functions.

Each terminal "session" corresponds to a workspace: a Kitty tab. Sessions are identified by an ID, have a human-readable name, and are rooted at a filesystem path.

## Dependencies

- `internal/process` -- for executing shell commands (`GetOutput`, `GetShellOutput`)

Configuration values are read by the caller (CLI or TUI layer) and passed as parameters:
- `terminal.session-creation-timeout-seconds` (global-only): integer, default `3`
- `terminal.default-app`: arbitrary shell command string (e.g. `"nvim"`). The value `"shell"` is a reserved sentinel meaning "launch a bare login/interactive shell without wrapping a command". Supplying a value that equals `$SHELL` (e.g. `/bin/zsh`) is detected and treated the same way.

## Prerequisites

Kitty's remote control feature must be enabled for Devora to manage terminal sessions. Add the following to your Kitty config (`~/.config/kitty/kitty.conf`):

```
allow_remote_control yes
```

Without this, all `kitty @` commands will fail.

## File Layout

```
internal/terminal/
  session.go         -- Session type, Backend interface, NewBackend factory, high-level functions
  kitty.go           -- KittyBackend implementation
  kitty_test.go      -- Tests for Kitty JSON parsing and session mapping
  session_test.go    -- Tests for high-level functions (GetSessionByWorkingDirectory, Attach, CreateAndAttach)
```

---

## Core Types

### Session

```go
type Session struct {
    ID       string
    Name     string
    RootPath string // absolute filesystem path
}
```

An immutable value type representing a terminal session. `RootPath` is always an absolute path without trailing slashes. It is used to associate sessions with workspaces by comparing against workspace directory paths.

### Backend

```go
type Backend interface {
    ListSessions() ([]Session, error)
    Attach(sessionID string) error
    CreateAndAttach(sessionName, workingDirectory, app string) error
}
```

The `Backend` interface defines the three operations every backend must support:

- **ListSessions**: Return all sessions visible from the current terminal context. May return an empty slice (not an error) when not running inside Kitty (e.g. `KITTY_WINDOW_ID` not set).
- **Attach**: Bring the session with the given ID into the foreground. For Kitty this means focusing a tab.
- **CreateAndAttach**: Create a new session with the given name, working directory, and application command. The session may not be immediately visible in `ListSessions` after this call returns (Kitty's `kitty @ launch` is asynchronous).

---

## CommandRunner

```go
type CommandRunner interface {
    GetOutput(command []string) (string, error)
    GetShellOutput(command string) (string, error)
}
```

`KittyBackend` stores a `CommandRunner` field. In production, this is backed by `process.GetOutput` / `process.GetShellOutput` via an unexported `productionCommandRunner` struct. In tests, a fake implementation returns canned output. The `NewBackend` factory wires up the production runner; tests construct backends directly with a fake runner.

## Backend Factory

```go
func NewBackend() Backend
```

Returns a `KittyBackend` wired with the production command runner (`process.GetOutput`/`process.GetShellOutput`) and `os.Getenv` for environment variable access. The TUI layer calls this to obtain a backend for session operations.

---

## High-Level Session Management Functions

These functions accept a `Backend` as an explicit parameter rather than relying on a module-level global.

### GetSessionByWorkingDirectory

```go
func GetSessionByWorkingDirectory(backend Backend, workingDirectory string) (*Session, error)
```

Calls `backend.ListSessions()` and returns the first session whose `RootPath` equals `workingDirectory` (exact string match). Returns `nil, nil` if no matching session is found. Returns `nil, err` if `ListSessions` fails.

### Attach

```go
func Attach(backend Backend, workingDirectory string) (*Session, error)
```

1. Call `GetSessionByWorkingDirectory(backend, workingDirectory)`.
2. If a session is found, call `backend.Attach(session.ID)`.
3. Return the session (or `nil` if none was found).
4. Propagate any error from either call.

### CreateAndAttach

```go
func CreateAndAttach(backend Backend, sessionName, workingDirectory, app string, timeoutSeconds int) (*Session, error)
```

1. Call `Attach(backend, workingDirectory)`. If a session is found (or an error occurs), return immediately.
2. Call `backend.CreateAndAttach(sessionName, workingDirectory, app)`. If this errors, return the error.
3. Poll for the new session:
   - Compute a deadline: `time.Now().Add(time.Duration(timeoutSeconds) * time.Second)`.
   - Loop until the deadline:
     - Call `GetSessionByWorkingDirectory(backend, workingDirectory)`.
     - If a session is found, return it.
     - Otherwise, sleep for 100 milliseconds (`time.Sleep(100 * time.Millisecond)`).
   - If the deadline expires without finding the session, return an error: `fmt.Errorf("failed to create session: timed out after %d seconds", timeoutSeconds)`.

The polling loop exists because session creation may be asynchronous. Kitty's `kitty @ launch` returns before the tab is fully initialized. The timeout is read from config (`terminal.session-creation-timeout-seconds`) by the caller and passed in explicitly.

---

## Kitty Backend

The Kitty backend uses Kitty's remote control protocol (`kitty @` commands) to manage tabs as sessions.

### Type

```go
type KittyBackend struct {
    Runner CommandRunner
    GetEnv EnvGetter
}
```

### KittyBackend.ListSessions

1. Read the `KITTY_WINDOW_ID` environment variable.
   - If not set or empty, return an empty slice (not an error). This means we are not running inside Kitty.
2. Parse `KITTY_WINDOW_ID` as an integer. If parsing fails, return an error.
3. Execute: `kitty @ ls` (via `process.GetShellOutput`).
4. Parse the stdout as JSON. The JSON structure is an array of OS window objects:
   ```json
   [
     {
       "id": 1,
       "tabs": [
         {
           "id": 10,
           "title": "my-workspace",
           "windows": [
             {
               "id": 100,
               "cwd": "/home/user/workspaces/ws-1"
             }
           ]
         }
       ]
     }
   ]
   ```
5. Find the OS window that contains a window matching `KITTY_WINDOW_ID`:
   - For each OS window, iterate its tabs.
   - For each tab, iterate its windows.
   - If any window's `"id"` matches the parsed `KITTY_WINDOW_ID` integer, use all tabs from that OS window.
   - If no match is found, return an empty slice.
6. Map each tab to a `Session`:
   - `ID`: the tab's `"id"` field, converted to a string via `strconv.Itoa`.
   - `Name`: the tab's `"title"` field.
   - `RootPath`: the `"cwd"` field of the first window (`windows[0]`) in the tab.

#### JSON Parsing Types

Define unexported types for deserializing the `kitty @ ls` output:

```go
type kittyOSWindow struct {
    ID   int        `json:"id"`
    Tabs []kittyTab `json:"tabs"`
}

type kittyTab struct {
    ID      int           `json:"id"`
    Title   string        `json:"title"`
    Windows []kittyWindow `json:"windows"`
}

type kittyWindow struct {
    ID  int    `json:"id"`
    CWD string `json:"cwd"`
}
```

#### Tab-Finding Logic

Extract a helper function:

```go
func findTabsForWindow(osWindows []kittyOSWindow, kittyWindowID int) []kittyTab
```

Iterates through all OS windows, their tabs, and their windows. When a window with a matching ID is found, returns all tabs from that OS window. Returns an empty slice if no match is found.

### KittyBackend.Attach

Execute: `kitty @ focus-tab --match id:<sessionID>` (via `process.GetShellOutput`).

The `sessionID` is the tab ID as a string. The command is constructed as a single shell string: `fmt.Sprintf("kitty @ focus-tab --match id:%s", sessionID)`.

### KittyBackend.CreateAndAttach

Execute via `process.GetOutput` with an argument list:

```go
args := []string{
    "kitty", "@", "launch",
    "--type=tab",
    "--tab-title", sessionName,
    fmt.Sprintf("--cwd=%s", workingDirectory),
    shell, "-l", "-i",
}
if app != "shell" && app != shell {
    args = append(args, "-c", app)
}
```

The shell is read from the `SHELL` environment variable (falling back to `/bin/sh` if unset). It is invoked with `-l -i` to ensure the user's shell profile is loaded. `-c <app>` is appended unless `app == "shell"` or `app == $SHELL`; in those two cases the shell is launched bare (no wrapped command) to avoid a redundant shell-in-shell. Otherwise the `app` parameter is an arbitrary command string passed as the argument to `<shell> -c`.

---

## Error Handling

- `ListSessions` returns `([]Session, error)`. The Kitty implementation returns an empty slice (not an error) when `KITTY_WINDOW_ID` is not set. Actual failures (JSON parse errors, unexpected output formats) are returned as errors.
- `Attach` and `CreateAndAttach` return errors from the underlying process execution.
- The high-level `CreateAndAttach` returns a timeout error if the session does not appear within the configured timeout.
- All errors from `process.GetOutput`/`process.GetShellOutput` are `*process.VerboseExecError` values containing the command, stdout, and stderr.

---

## Testing

### Kitty JSON Parsing (`kitty_test.go`)

Test `findTabsForWindow` and the `ListSessions` parsing logic using fixture JSON data. Do NOT call actual `kitty @` commands.

**Test cases:**

1. **Single OS window, multiple tabs**: Provide JSON with one OS window containing 3 tabs. Set `KITTY_WINDOW_ID` to a window in one of the tabs. Verify all 3 tabs are returned as sessions with correct ID, Name, and RootPath.

2. **Multiple OS windows**: Provide JSON with two OS windows. Set `KITTY_WINDOW_ID` to a window in the second OS window. Verify only tabs from the second OS window are returned.

3. **Window ID not found**: Provide JSON where no window matches the `KITTY_WINDOW_ID`. Verify an empty slice is returned.

4. **KITTY_WINDOW_ID not set**: Verify `ListSessions` returns an empty slice without executing any commands.

5. **Tab ID is converted to string**: Verify that integer tab IDs from JSON are correctly converted to string session IDs.

6. **RootPath is taken from first window's cwd**: For a tab with multiple windows, verify `RootPath` comes from `windows[0].cwd`.

### High-Level Functions (`session_test.go`)

Test using a mock `Backend` implementation (a struct satisfying the `Backend` interface that records calls and returns configured data).

**Test cases:**

1. **GetSessionByWorkingDirectory -- found**: Backend returns sessions including one matching the directory. Verify the correct session is returned.

2. **GetSessionByWorkingDirectory -- not found**: Backend returns sessions, none matching. Verify `nil` is returned.

3. **GetSessionByWorkingDirectory -- empty list**: Backend returns empty slice. Verify `nil` is returned.

4. **Attach -- session exists**: Verify `Attach` calls `backend.Attach` with the correct session ID and returns the session.

5. **Attach -- no session**: Verify `Attach` returns `nil` without calling `backend.Attach`.

6. **CreateAndAttach -- session already exists**: Verify it attaches to the existing session without calling `backend.CreateAndAttach`.

7. **CreateAndAttach -- session created successfully**: Backend returns no session on first calls, then returns a session after `CreateAndAttach` is called. Verify the polling loop finds the session.

8. **CreateAndAttach -- timeout**: Backend never returns a matching session. Verify a timeout error is returned.

9. **NewBackend**: Verify `NewBackend()` returns a `*KittyBackend`.

### Process Execution Testability

Tests construct backends directly with a fake `CommandRunner` that returns canned output, rather than using `NewBackend` which wires up the production runner.

### Environment Variable Testability

`KittyBackend.ListSessions` reads `KITTY_WINDOW_ID`. The backend accepts an `EnvGetter` function (`func(key string) string`). In production (via `NewBackend`), this is `os.Getenv`. In tests, it returns controlled values.

---

## Summary of Public API

```go
// Types
type Session struct { ID, Name, RootPath string }
type Backend interface { ListSessions, Attach, CreateAndAttach }
type CommandRunner interface { GetOutput, GetShellOutput }
type EnvGetter func(key string) string

// Factory
func NewBackend() Backend

// High-level functions
func GetSessionByWorkingDirectory(backend Backend, workingDirectory string) (*Session, error)
func Attach(backend Backend, workingDirectory string) (*Session, error)
func CreateAndAttach(backend Backend, sessionName, workingDirectory, app string, timeoutSeconds int) (*Session, error)

// Backend implementation (constructed by NewBackend, or directly in tests)
type KittyBackend struct { Runner CommandRunner; GetEnv EnvGetter }
```
