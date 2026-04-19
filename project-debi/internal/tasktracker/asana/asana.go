// Package asana is the Asana provider for internal/tasktracker. It talks to
// the Asana REST API at https://app.asana.com/api/1.0 with exactly three
// endpoints: GET /users/me, POST /workspaces/<ws>/tasks, and PUT /tasks/<id>.
//
// The package registers itself with tasktracker.Register("asana", New) from
// init(), so callers that need the provider must side-effect-import it:
//
//	import _ "devora/internal/tasktracker/asana"
package asana

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"devora/internal/config"
	"devora/internal/credentials"
	"devora/internal/tasktracker"
)

const (
	baseURL      = "https://app.asana.com/api/1.0"
	providerName = "asana"
)

// Config is the resolved provider config from task-tracker.asana.*.
// All fields except SectionID are required.
type Config struct {
	WorkspaceID string
	ProjectID   string
	CLITag      string
	SectionID   string // optional
}

// tracker is the Tracker implementation for Asana.
type tracker struct {
	cfg         Config
	client      *http.Client
	cachedToken string
}

// Stubbable package-level vars — tests replace these to avoid real network
// and keychain access. httpDo takes the full request so tests can assert on
// URL, method, headers, and body.
var (
	httpDo        = defaultHTTPDo
	getCredential = credentials.GetToken
)

func defaultHTTPDo(req *http.Request) (*http.Response, error) {
	c := &http.Client{Timeout: 30 * time.Second}
	return c.Do(req)
}

func init() {
	tasktracker.Register(providerName, New)
}

// New is the factory registered with tasktracker.Register("asana", New). It
// reads config eagerly and errors if any required key is missing, listing
// the missing keys. Credentials are fetched lazily on the first API call.
func New() (tasktracker.Tracker, error) {
	cfg := Config{
		WorkspaceID: config.GetTaskTrackerString(providerName, "workspace-id"),
		ProjectID:   config.GetTaskTrackerString(providerName, "project-id"),
		CLITag:      config.GetTaskTrackerString(providerName, "cli-tag"),
		SectionID:   config.GetTaskTrackerString(providerName, "section-id"),
	}

	var missing []string
	if cfg.WorkspaceID == "" {
		missing = append(missing, "workspace-id")
	}
	if cfg.ProjectID == "" {
		missing = append(missing, "project-id")
	}
	if cfg.CLITag == "" {
		missing = append(missing, "cli-tag")
	}
	if len(missing) > 0 {
		return nil, fmt.Errorf("asana task-tracker config incomplete: missing %s", strings.Join(missing, ", "))
	}

	return &tracker{cfg: cfg}, nil
}

// --- Tracker interface ---

func (t *tracker) Provider() string { return providerName }

// WhoAmI returns the authenticated user's Asana GID.
func (t *tracker) WhoAmI() (string, error) {
	var resp struct {
		Data struct {
			GID string `json:"gid"`
		} `json:"data"`
	}
	if err := t.do(http.MethodGet, "/users/me", nil, &resp); err != nil {
		return "", err
	}
	return resp.Data.GID, nil
}

// CreateTask creates an Asana task in the configured workspace and project
// (and section, when set). The memberships array carries project + section
// so no separate addProject call is needed.
func (t *tracker) CreateTask(req tasktracker.CreateTaskRequest) (tasktracker.Task, error) {
	membership := map[string]string{"project": t.cfg.ProjectID}
	if t.cfg.SectionID != "" {
		membership["section"] = t.cfg.SectionID
	}

	body := map[string]any{
		"data": map[string]any{
			"name":        req.Title,
			"notes":       "",
			"assignee":    req.AssigneeGID,
			"memberships": []map[string]string{membership},
			"tags":        []string{t.cfg.CLITag},
		},
	}

	var resp struct {
		Data struct {
			GID  string `json:"gid"`
			Name string `json:"name"`
		} `json:"data"`
	}
	path := "/workspaces/" + t.cfg.WorkspaceID + "/tasks"
	if err := t.do(http.MethodPost, path, body, &resp); err != nil {
		return tasktracker.Task{}, err
	}
	return tasktracker.Task{
		ID:   resp.Data.GID,
		Name: resp.Data.Name,
		URL:  t.TaskURL(resp.Data.GID),
	}, nil
}

// CompleteTask marks the task with the given ID as completed.
func (t *tracker) CompleteTask(taskID string) error {
	body := map[string]any{
		"data": map[string]any{"completed": true},
	}
	return t.do(http.MethodPut, "/tasks/"+taskID, body, nil)
}

// ParseTaskURL delegates to the package-level ParseTaskURL (in parse_url.go)
// so the URL parsing code is unit-testable without constructing a tracker.
func (t *tracker) ParseTaskURL(url string) string {
	return ParseTaskURL(url)
}

// TaskURL returns the V1 web URL for the given task, using the tracker's
// configured workspace. Mirrors fdev's GetTaskURL at
// fdev/internal/api/asana/client.go:192-194.
func (t *tracker) TaskURL(taskID string) string {
	return fmt.Sprintf("https://app.asana.com/1/%s/task/%s", t.cfg.WorkspaceID, taskID)
}

// PRBodyPrefix returns the line included in PR bodies to link to the task.
func (t *tracker) PRBodyPrefix(taskID string) string {
	return "asana: " + t.TaskURL(taskID)
}

// --- HTTP helpers ---

// token returns the cached Asana token, fetching it on first use. A cached
// empty value triggers a re-fetch — this keeps the "credential went missing
// mid-session" case recoverable without paying the keychain cost every call.
func (t *tracker) token() (string, error) {
	if t.cachedToken != "" {
		return t.cachedToken, nil
	}
	tok, err := getCredential(providerName)
	if err != nil {
		return "", err
	}
	t.cachedToken = tok
	return tok, nil
}

// do performs an Asana API request. It wraps JSON encoding of the body,
// setting the required headers, decoding the response into out (when
// non-nil), and turning 4xx/5xx into an "asana API error" error to match
// fdev's style at fdev/internal/api/asana/client.go:248-250.
func (t *tracker) do(method, path string, body any, out any) error {
	tok, err := t.token()
	if err != nil {
		return err
	}

	var bodyReader io.Reader
	if body != nil {
		b, err := json.Marshal(body)
		if err != nil {
			return fmt.Errorf("marshal asana request: %w", err)
		}
		bodyReader = bytes.NewReader(b)
	}

	req, err := http.NewRequest(method, baseURL+path, bodyReader)
	if err != nil {
		return fmt.Errorf("build asana request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+tok)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Asana-Enable", "string_ids,new_rich_text")

	resp, err := httpDo(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {
		b, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20)) // 1MB max
		return fmt.Errorf("asana API error (status %d): %s", resp.StatusCode, string(b))
	}

	if out != nil {
		if err := json.NewDecoder(resp.Body).Decode(out); err != nil {
			return fmt.Errorf("decode asana response: %w", err)
		}
	}
	return nil
}
