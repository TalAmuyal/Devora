# Task Tracker Package Spec

Package: `internal/tasktracker`

## Purpose

Define the pluggable task-tracker interface used by `submit`, `close`, and `health`, plus a minimal registry of provider factories. Provider implementations live in subpackages (e.g., `internal/tasktracker/asana`) and register themselves via `init()`.

## Stability

Stable public API: `Tracker` interface, `Task`, `CreateTaskRequest`, `Factory`, `Register`, `NewForActiveProfile`.

## Dependencies

- `devora/internal/config` -- reads `task-tracker.provider`

## Types

### Task

```go
type Task struct {
    ID   string
    Name string
    URL  string
}
```

The minimal shape both submit (create) and close (complete) need. `ID` is the provider's canonical identifier (e.g., an Asana GID). `URL` is the user-facing web URL.

### CreateTaskRequest

```go
type CreateTaskRequest struct {
    Title       string
    AssigneeGID string // optional; "" means no assignee
}
```

### Factory

```go
type Factory func() (Tracker, error)
```

Constructs a `Tracker` from config. Must read config eagerly and return an error when required config is missing or invalid. Must not fetch credentials; those are fetched lazily on the first API call so `New()` does not touch the keychain.

## Tracker Interface

```go
type Tracker interface {
    Provider() string                                      // "asana", etc. Must match config value.
    WhoAmI() (string, error)                               // provider-specific self-id (for assignee).
    CreateTask(req CreateTaskRequest) (Task, error)        // creates a task; returns canonical ID and URL.
    CompleteTask(taskID string) error                      // marks the task complete.
    ParseTaskURL(url string) string                        // offline; "" when URL is unrecognized.
    TaskURL(taskID string) string                          // offline; canonical web URL for ID.
    PRBodyPrefix(taskID string) string                     // line included in PR body, e.g. "asana: <url>".
}
```

Only a provider that is configured *and* successfully constructed can satisfy this interface. "No provider configured" is represented by `(nil, nil)` from `NewForActiveProfile`.

`ParseTaskURL` and `TaskURL` are offline (no network) so `close --task-url` stays fast, and submit's PR-body composition does not require a round-trip.

## Registry

```go
var factories = map[string]Factory{}

func Register(name string, f Factory)
```

`Register` is called from each provider's `init()`. The registry is an unsynchronized map because `init()` functions run serially at program startup. `Register` panics when a factory is already registered under the same name, surfacing accidental duplicate registrations at startup rather than silently overwriting.

Provider implementations must be blank-imported by packages that need them. `internal/cli/cli.go` performs the Asana registration:

```go
// Register the Asana task-tracker provider. The blank import runs the
// package's init(), which calls tasktracker.Register("asana", New).
_ "devora/internal/tasktracker/asana"
```

Packages that depend on `internal/cli` (i.e. `main.go`) therefore get the Asana provider transitively. Domain packages (`submit`, `closecmd`, `health`) do not import the provider directly -- they go through `NewForActiveProfile`.

## Functions

### NewForActiveProfile

```go
func NewForActiveProfile() (Tracker, error)
```

Resolves the configured provider via `config.GetTaskTrackerProvider()` and dispatches to its factory.

- `task-tracker.provider` unset or `""` -> `(nil, nil)`. Callers check `if tracker == nil`.
- Known provider -> calls the factory. Missing-config errors from the factory propagate.
- Unknown provider -> returns `fmt.Errorf("unknown task-tracker provider: %s", provider)`.

Credentials are fetched lazily by the provider on its first network call; `NewForActiveProfile` does not touch the keychain.

## Stubbable Dependencies

```go
var getProvider = config.GetTaskTrackerProvider
```

Tests replace `getProvider` to avoid setting up config fixtures.

## Config Shape

```json
{
    "task-tracker": {
        "provider": "asana",
        "asana": {
            "workspace-id": "YOUR-WORKSPACE-GID",
            "cli-tag": "YOUR-CLI-TAG-GID",
            "project-id": "YOUR-PROJECT-GID",
            "section-id": "YOUR-SECTION-GID"
        }
    }
}
```

- `task-tracker.provider` selects the provider (profile-overridable).
- `task-tracker.<provider>.<key>` holds provider-specific settings (profile-overridable per leaf: profile and global can each contribute different leaves).

See [config.md](config.md#gettasktrackerprovider) for the config-resolution semantics. The Asana provider requires `workspace-id`, `project-id`, `cli-tag`; `section-id` is optional.

## Test Coverage

`internal/tasktracker/tasktracker_test.go` covers:

- No provider configured returns `(nil, nil)`.
- Known provider returns the factory-built tracker.
- Unknown provider returns an error mentioning the name.
- Factory errors propagate.
- Duplicate `Register` call for the same name panics.
