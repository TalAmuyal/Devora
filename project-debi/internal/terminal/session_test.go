package terminal

import (
	"fmt"
	"testing"
)

// mockBackend is a test double that records calls and returns configured data.
type mockBackend struct {
	sessions         []Session
	listSessionsErr  error
	attachErr        error
	createAttachErr  error
	attachedID       string
	createCalls      []createCall
	listCallCount    int
	sessionsSequence [][]Session // if set, listCallCount indexes into this for successive calls
}

type createCall struct {
	name, dir, app string
}

func (m *mockBackend) ListSessions() ([]Session, error) {
	idx := m.listCallCount
	m.listCallCount++
	if m.sessionsSequence != nil && idx < len(m.sessionsSequence) {
		return m.sessionsSequence[idx], m.listSessionsErr
	}
	return m.sessions, m.listSessionsErr
}

func (m *mockBackend) Attach(sessionID string) error {
	m.attachedID = sessionID
	return m.attachErr
}

func (m *mockBackend) CreateAndAttach(sessionName, workingDirectory, app string) error {
	m.createCalls = append(m.createCalls, createCall{sessionName, workingDirectory, app})
	return m.createAttachErr
}

func TestGetSessionByWorkingDirectory_Found(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{
			{ID: "1", Name: "ws-1", RootPath: "/home/user/ws-1"},
			{ID: "2", Name: "ws-2", RootPath: "/home/user/ws-2"},
			{ID: "3", Name: "ws-3", RootPath: "/home/user/ws-3"},
		},
	}

	session, err := GetSessionByWorkingDirectory(backend, "/home/user/ws-2")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session == nil {
		t.Fatal("expected a session, got nil")
	}
	if session.ID != "2" {
		t.Errorf("expected ID %q, got %q", "2", session.ID)
	}
	if session.Name != "ws-2" {
		t.Errorf("expected Name %q, got %q", "ws-2", session.Name)
	}
}

func TestGetSessionByWorkingDirectory_NotFound(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{
			{ID: "1", Name: "ws-1", RootPath: "/home/user/ws-1"},
		},
	}

	session, err := GetSessionByWorkingDirectory(backend, "/home/user/ws-999")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session != nil {
		t.Fatalf("expected nil, got %+v", session)
	}
}

func TestGetSessionByWorkingDirectory_EmptyList(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{},
	}

	session, err := GetSessionByWorkingDirectory(backend, "/home/user/ws")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session != nil {
		t.Fatalf("expected nil, got %+v", session)
	}
}

func TestAttach_SessionExists(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{
			{ID: "42", Name: "ws", RootPath: "/home/user/ws"},
		},
	}

	session, err := Attach(backend, "/home/user/ws")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session == nil {
		t.Fatal("expected a session, got nil")
	}
	if session.ID != "42" {
		t.Errorf("expected ID %q, got %q", "42", session.ID)
	}
	if backend.attachedID != "42" {
		t.Errorf("expected Attach called with %q, got %q", "42", backend.attachedID)
	}
}

func TestAttach_NoSession(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{},
	}

	session, err := Attach(backend, "/home/user/ws")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session != nil {
		t.Fatalf("expected nil, got %+v", session)
	}
	if backend.attachedID != "" {
		t.Errorf("expected Attach not called, but was called with %q", backend.attachedID)
	}
}

func TestCreateAndAttach_SessionAlreadyExists(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{
			{ID: "42", Name: "ws", RootPath: "/home/user/ws"},
		},
	}

	session, err := CreateAndAttach(backend, "ws", "/home/user/ws", "nvim", 3)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session == nil {
		t.Fatal("expected a session, got nil")
	}
	if session.ID != "42" {
		t.Errorf("expected ID %q, got %q", "42", session.ID)
	}
	if len(backend.createCalls) != 0 {
		t.Errorf("expected no CreateAndAttach calls, got %d", len(backend.createCalls))
	}
}

func TestCreateAndAttach_CreatedSuccessfully(t *testing.T) {
	createdSession := Session{ID: "99", Name: "new-ws", RootPath: "/home/user/new-ws"}
	backend := &mockBackend{
		sessionsSequence: [][]Session{
			{}, // first ListSessions call from Attach -> GetSessionByWorkingDirectory
			{createdSession}, // second call from polling
		},
	}

	session, err := CreateAndAttach(backend, "new-ws", "/home/user/new-ws", "nvim", 3)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if session == nil {
		t.Fatal("expected a session, got nil")
	}
	if session.ID != "99" {
		t.Errorf("expected ID %q, got %q", "99", session.ID)
	}
	if len(backend.createCalls) != 1 {
		t.Fatalf("expected 1 CreateAndAttach call, got %d", len(backend.createCalls))
	}
	call := backend.createCalls[0]
	if call.name != "new-ws" || call.dir != "/home/user/new-ws" || call.app != "nvim" {
		t.Errorf("unexpected CreateAndAttach args: %+v", call)
	}
}

func TestCreateAndAttach_Timeout(t *testing.T) {
	backend := &mockBackend{
		sessions: []Session{}, // never returns a matching session
	}

	session, err := CreateAndAttach(backend, "ws", "/home/user/ws", "nvim", 1)
	if err == nil {
		t.Fatal("expected timeout error, got nil")
	}
	if session != nil {
		t.Fatalf("expected nil session, got %+v", session)
	}
	expected := fmt.Sprintf("failed to create session: timed out after %d seconds", 1)
	if err.Error() != expected {
		t.Errorf("expected error %q, got %q", expected, err.Error())
	}
}

func TestNewBackend(t *testing.T) {
	backend := NewBackend()
	if _, ok := backend.(*KittyBackend); !ok {
		t.Errorf("expected *KittyBackend, got %T", backend)
	}
}
