package asana

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"

	"devora/internal/config"
	"devora/internal/credentials"
	"devora/internal/tasktracker"
)

// stubAsanaDeps captures and restores the overridable package-level vars so
// tests can stub them without leaking state.
func stubAsanaDeps(t *testing.T) {
	t.Helper()
	origHTTPDo := httpDo
	origGetCredential := getCredential
	t.Cleanup(func() {
		httpDo = origHTTPDo
		getCredential = origGetCredential
	})
}

// withConfigFile writes a minimal Devora global config at a temp path and
// points config.ConfigPath() at it for the duration of the test.
func withConfigFile(t *testing.T, contents string) {
	t.Helper()
	tmp := filepath.Join(t.TempDir(), "config.json")
	if err := os.WriteFile(tmp, []byte(contents), 0o644); err != nil {
		t.Fatalf("write temp config: %v", err)
	}
	config.SetConfigPathForTesting(tmp)
	t.Cleanup(func() {
		config.ResetForTesting()
	})
}

// --- helpers for building stub http.Response values ---

func jsonResponse(status int, body any) *http.Response {
	b, _ := json.Marshal(body)
	return &http.Response{
		StatusCode: status,
		Body:       io.NopCloser(bytes.NewReader(b)),
		Header:     make(http.Header),
	}
}

func rawResponse(status int, body string) *http.Response {
	return &http.Response{
		StatusCode: status,
		Body:       io.NopCloser(strings.NewReader(body)),
		Header:     make(http.Header),
	}
}

// fullyConfigured is the JSON fixture used for tests that want all required
// keys present. SectionID is included so the "with section" test path is
// covered by the default fixture; "without section" tests override as needed.
const fullyConfiguredJSON = `{
    "task-tracker": {
        "asana": {
            "workspace-id": "WS-1",
            "project-id": "PROJ-1",
            "cli-tag": "TAG-1",
            "section-id": "SEC-1"
        }
    }
}`

const withoutSectionJSON = `{
    "task-tracker": {
        "asana": {
            "workspace-id": "WS-1",
            "project-id": "PROJ-1",
            "cli-tag": "TAG-1"
        }
    }
}`

// --- New() config validation ---

func TestNew_MissingAllRequired_ReturnsErrorListingKeys(t *testing.T) {
	withConfigFile(t, `{}`)

	_, err := New()
	if err == nil {
		t.Fatal("expected error for missing config")
	}
	msg := err.Error()
	for _, key := range []string{"workspace-id", "project-id", "cli-tag"} {
		if !strings.Contains(msg, key) {
			t.Errorf("expected error to mention %q, got %q", key, msg)
		}
	}
	if !strings.Contains(msg, "asana task-tracker config incomplete") {
		t.Errorf("expected prefix 'asana task-tracker config incomplete', got %q", msg)
	}
}

func TestNew_MissingOneRequired_ReturnsErrorListingThatKey(t *testing.T) {
	withConfigFile(t, `{
        "task-tracker": {
            "asana": {
                "workspace-id": "W",
                "cli-tag": "T"
            }
        }
    }`)

	_, err := New()
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "project-id") {
		t.Errorf("expected error to mention project-id, got %q", err.Error())
	}
	if strings.Contains(err.Error(), "workspace-id") {
		t.Errorf("did not expect workspace-id in error, got %q", err.Error())
	}
}

func TestNew_SectionIDOptional_AllRequiredPresent_Succeeds(t *testing.T) {
	withConfigFile(t, withoutSectionJSON)

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tr == nil {
		t.Fatal("expected tracker")
	}
	if tr.Provider() != "asana" {
		t.Fatalf("expected provider 'asana', got %q", tr.Provider())
	}
}

// --- Provider / TaskURL / PRBodyPrefix ---

func TestProvider_ReturnsAsana(t *testing.T) {
	withConfigFile(t, fullyConfiguredJSON)
	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tr.Provider() != "asana" {
		t.Fatalf("expected 'asana', got %q", tr.Provider())
	}
}

func TestPRBodyPrefix_IncludesTaskURL(t *testing.T) {
	withConfigFile(t, fullyConfiguredJSON)
	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	got := tr.PRBodyPrefix("123")
	want := "asana: https://app.asana.com/1/WS-1/task/123"
	if got != want {
		t.Fatalf("expected %q, got %q", want, got)
	}
}

// --- WhoAmI ---

func TestWhoAmI_ReturnsGIDFromResponse(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, fullyConfiguredJSON)

	getCredential = func(provider string) (string, error) {
		if provider != "asana" {
			t.Fatalf("expected provider 'asana', got %q", provider)
		}
		return "TOKEN", nil
	}
	httpDo = func(req *http.Request) (*http.Response, error) {
		if req.Method != http.MethodGet {
			t.Fatalf("expected GET, got %s", req.Method)
		}
		if req.URL.String() != "https://app.asana.com/api/1.0/users/me" {
			t.Fatalf("unexpected URL: %s", req.URL.String())
		}
		if got := req.Header.Get("Authorization"); got != "Bearer TOKEN" {
			t.Fatalf("expected Bearer TOKEN, got %q", got)
		}
		if got := req.Header.Get("Asana-Enable"); got != "string_ids,new_rich_text" {
			t.Fatalf("unexpected Asana-Enable: %q", got)
		}
		if got := req.Header.Get("Accept"); got != "application/json" {
			t.Fatalf("unexpected Accept: %q", got)
		}
		return jsonResponse(200, map[string]any{"data": map[string]any{"gid": "123"}}), nil
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	gid, err := tr.WhoAmI()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gid != "123" {
		t.Fatalf("expected gid '123', got %q", gid)
	}
}

// --- CreateTask payload shape ---

func TestCreateTask_PayloadWithSectionID(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, fullyConfiguredJSON)

	var capturedBody []byte
	getCredential = func(string) (string, error) { return "TOKEN", nil }
	httpDo = func(req *http.Request) (*http.Response, error) {
		if req.Method != http.MethodPost {
			t.Fatalf("expected POST, got %s", req.Method)
		}
		if req.URL.String() != "https://app.asana.com/api/1.0/workspaces/WS-1/tasks" {
			t.Fatalf("unexpected URL: %s", req.URL.String())
		}
		if req.Header.Get("Content-Type") != "application/json" {
			t.Fatalf("unexpected Content-Type: %q", req.Header.Get("Content-Type"))
		}
		b, err := io.ReadAll(req.Body)
		if err != nil {
			t.Fatalf("read body: %v", err)
		}
		capturedBody = b
		return jsonResponse(201, map[string]any{
			"data": map[string]any{"gid": "NEW-TASK", "name": "My title"},
		}), nil
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	task, err := tr.CreateTask(tasktracker.CreateTaskRequest{
		Title:       "My title",
		AssigneeGID: "USER-1",
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	var actual map[string]any
	if err := json.Unmarshal(capturedBody, &actual); err != nil {
		t.Fatalf("unmarshal body: %v", err)
	}
	expected := map[string]any{
		"data": map[string]any{
			"name":     "My title",
			"notes":    "",
			"assignee": "USER-1",
			"memberships": []any{
				map[string]any{"project": "PROJ-1", "section": "SEC-1"},
			},
			"tags": []any{"TAG-1"},
		},
	}
	if !jsonEqual(actual, expected) {
		t.Fatalf("payload mismatch\n got: %s\nwant: %s", string(capturedBody), mustMarshal(expected))
	}

	if task.ID != "NEW-TASK" {
		t.Errorf("expected ID 'NEW-TASK', got %q", task.ID)
	}
	if task.Name != "My title" {
		t.Errorf("expected Name 'My title', got %q", task.Name)
	}
	if task.URL != "https://app.asana.com/1/WS-1/task/NEW-TASK" {
		t.Errorf("unexpected URL: %q", task.URL)
	}
}

func TestCreateTask_PayloadWithoutSectionID_OmitsSectionKey(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, withoutSectionJSON)

	var capturedBody []byte
	getCredential = func(string) (string, error) { return "TOKEN", nil }
	httpDo = func(req *http.Request) (*http.Response, error) {
		b, _ := io.ReadAll(req.Body)
		capturedBody = b
		return jsonResponse(201, map[string]any{
			"data": map[string]any{"gid": "T", "name": "T"},
		}), nil
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, err := tr.CreateTask(tasktracker.CreateTaskRequest{Title: "T", AssigneeGID: "U"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	var actual map[string]any
	if err := json.Unmarshal(capturedBody, &actual); err != nil {
		t.Fatalf("unmarshal body: %v", err)
	}
	data := actual["data"].(map[string]any)
	memberships := data["memberships"].([]any)
	if len(memberships) != 1 {
		t.Fatalf("expected 1 membership, got %d", len(memberships))
	}
	m := memberships[0].(map[string]any)
	if m["project"] != "PROJ-1" {
		t.Errorf("expected project PROJ-1, got %v", m["project"])
	}
	if _, hasSection := m["section"]; hasSection {
		t.Errorf("expected no 'section' key when SectionID is empty, got %v", m["section"])
	}
}

// --- CompleteTask ---

func TestCompleteTask_PUTsCompletedTrue(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, fullyConfiguredJSON)

	var capturedBody []byte
	getCredential = func(string) (string, error) { return "TOKEN", nil }
	httpDo = func(req *http.Request) (*http.Response, error) {
		if req.Method != http.MethodPut {
			t.Fatalf("expected PUT, got %s", req.Method)
		}
		if req.URL.String() != "https://app.asana.com/api/1.0/tasks/ABC" {
			t.Fatalf("unexpected URL: %s", req.URL.String())
		}
		b, _ := io.ReadAll(req.Body)
		capturedBody = b
		return rawResponse(200, `{"data": {"gid": "ABC"}}`), nil
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if err := tr.CompleteTask("ABC"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	var actual map[string]any
	if err := json.Unmarshal(capturedBody, &actual); err != nil {
		t.Fatalf("unmarshal body: %v", err)
	}
	expected := map[string]any{
		"data": map[string]any{"completed": true},
	}
	if !jsonEqual(actual, expected) {
		t.Fatalf("payload mismatch\n got: %s\nwant: %s", string(capturedBody), mustMarshal(expected))
	}
}

// --- HTTP errors ---

func TestHTTP_4xx_ReturnsAsanaAPIError(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, fullyConfiguredJSON)

	getCredential = func(string) (string, error) { return "TOKEN", nil }
	httpDo = func(req *http.Request) (*http.Response, error) {
		return rawResponse(404, `{"error":"not found"}`), nil
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	_, err = tr.WhoAmI()
	if err == nil {
		t.Fatal("expected error")
	}
	msg := err.Error()
	if !strings.Contains(msg, "asana API error (status 404)") {
		t.Errorf("expected 'asana API error (status 404)' in %q", msg)
	}
	if !strings.Contains(msg, `{"error":"not found"}`) {
		t.Errorf("expected body in error, got %q", msg)
	}
}

// --- Credential missing propagates NotFoundError ---

func TestCredentialMissing_NotFoundErrorPropagates(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, fullyConfiguredJSON)

	notFound := &credentials.NotFoundError{
		Provider: "asana",
		Service:  "devora-asana",
		Account:  "alice",
	}
	getCredential = func(string) (string, error) { return "", notFound }
	httpDo = func(req *http.Request) (*http.Response, error) {
		t.Fatal("httpDo must not be called when credentials are missing")
		return nil, nil
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	_, err = tr.WhoAmI()
	if err == nil {
		t.Fatal("expected error")
	}
	var nf *credentials.NotFoundError
	if !errors.As(err, &nf) {
		t.Fatalf("expected *credentials.NotFoundError in chain, got %T: %v", err, err)
	}
}

// --- Token caching ---

func TestToken_CachedAcrossCalls(t *testing.T) {
	stubAsanaDeps(t)
	withConfigFile(t, fullyConfiguredJSON)

	var getCredCalls int32
	getCredential = func(string) (string, error) {
		atomic.AddInt32(&getCredCalls, 1)
		return "TOKEN", nil
	}
	httpDo = func(req *http.Request) (*http.Response, error) {
		// Respond as needed for whichever method the test dispatches.
		switch {
		case strings.HasSuffix(req.URL.Path, "/users/me"):
			return jsonResponse(200, map[string]any{"data": map[string]any{"gid": "ME"}}), nil
		default:
			return rawResponse(200, `{"data":{"gid":"T","name":"T"}}`), nil
		}
	}

	tr, err := New()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, err := tr.WhoAmI(); err != nil {
		t.Fatalf("WhoAmI: %v", err)
	}
	if err := tr.CompleteTask("X"); err != nil {
		t.Fatalf("CompleteTask: %v", err)
	}

	got := atomic.LoadInt32(&getCredCalls)
	if got != 1 {
		t.Fatalf("expected getCredential to be called exactly once, got %d", got)
	}
}

// --- Registration ---

func TestInit_RegistersAsanaFactory(t *testing.T) {
	// The package init() should register the "asana" factory. Attempting to
	// register it again must panic (per tasktracker.Register semantics).
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("expected panic on duplicate registration")
		}
		msg, ok := r.(string)
		if !ok {
			t.Fatalf("unexpected panic payload: %T %v", r, r)
		}
		if !strings.Contains(msg, "asana") {
			t.Fatalf("expected panic to mention 'asana', got %q", msg)
		}
	}()
	tasktracker.Register("asana", New)
}

// --- helpers ---

func jsonEqual(a, b any) bool {
	return string(mustMarshal(a)) == string(mustMarshal(b))
}

func mustMarshal(v any) []byte {
	b, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return b
}
