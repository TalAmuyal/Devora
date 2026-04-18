// Package tasktracker defines the pluggable issue-tracker interface used by
// submit/close/health, along with a tiny registry of provider factories.
// Provider implementations live in subpackages (e.g.,
// internal/tasktracker/asana) and register themselves from init().
package tasktracker

import (
	"fmt"

	"devora/internal/config"
)

// Task is the minimal shape both submit (create) and close (complete) need.
type Task struct {
	ID   string
	Name string
	URL  string
}

// CreateTaskRequest bundles inputs for CreateTask.
type CreateTaskRequest struct {
	Title string
	// AssigneeGID is optional. Providers that have no notion of "self" may
	// ignore it. The empty string means "no assignee".
	AssigneeGID string
}

// Tracker is the pluggable interface. Only a tracker that is *configured and
// successfully constructed* can satisfy this interface.
type Tracker interface {
	// Provider returns the provider key (e.g., "asana"). Must match the
	// value used in config at task-tracker.provider.
	Provider() string

	// WhoAmI returns the authenticated user's provider-specific GID/ID.
	// Called by submit to set task assignee.
	WhoAmI() (string, error)

	// CreateTask creates a task. The returned Task.ID is the provider's
	// canonical ID; Task.URL is the user-facing web URL.
	CreateTask(req CreateTaskRequest) (Task, error)

	// CompleteTask marks the task with the given ID as complete.
	CompleteTask(taskID string) error

	// ParseTaskURL extracts a task ID from a user-supplied URL. Returns ""
	// when the URL is not recognized. Offline / no network.
	ParseTaskURL(url string) string

	// TaskURL returns the canonical web URL for the given ID. Offline.
	TaskURL(taskID string) string

	// PRBodyPrefix returns the first line included in the PR body when a
	// tracker is configured (e.g., "asana: https://app.asana.com/...").
	PRBodyPrefix(taskID string) string
}

// Factory constructs a Tracker from config. It reads config eagerly and
// returns an error when required config is missing or invalid. It must not
// fetch credentials — those are fetched lazily by the provider on the first
// API call.
type Factory func() (Tracker, error)

// factories holds the registered provider factories, keyed by provider name.
// Registration happens in provider init() functions, which run serially at
// program startup, so no mutex is needed.
var factories = map[string]Factory{}

// getProvider is the stubbable entry point for reading the configured
// provider. Tests replace it to avoid setting up config fixtures.
var getProvider = config.GetTaskTrackerProvider

// Register adds a provider factory to the registry. It panics when a factory
// is already registered under the same name — this surfaces duplicate
// registrations (e.g., an accidental double-import) at startup rather than
// silently overwriting the existing factory.
func Register(name string, f Factory) {
	if _, exists := factories[name]; exists {
		panic(fmt.Sprintf("tasktracker: provider %q is already registered", name))
	}
	factories[name] = f
}

// NewForActiveProfile constructs a Tracker from the active-profile config:
//
//   - task-tracker.provider unset or "": returns (nil, nil). No tracker is
//     configured; callers check `if tracker == nil`.
//   - task-tracker.provider == <known>:  calls the registered factory. If the
//     factory returns an error (e.g., missing required config), the error is
//     propagated.
//   - task-tracker.provider == <unknown>: returns an error that mentions the
//     unknown provider name.
//
// Credentials are fetched lazily by the provider on its first network call.
func NewForActiveProfile() (Tracker, error) {
	provider := getProvider()
	if provider == "" {
		return nil, nil
	}
	factory, ok := factories[provider]
	if !ok {
		return nil, fmt.Errorf("unknown task-tracker provider: %s", provider)
	}
	return factory()
}
