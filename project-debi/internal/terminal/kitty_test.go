package terminal

import (
	"fmt"
	"testing"
)

type fakeCommandRunner struct {
	shellOutputs map[string]string
	shellErrors  map[string]error
	outputs      map[string]string
	errors       map[string]error
	outputCalls  [][]string
}

func newFakeCommandRunner() *fakeCommandRunner {
	return &fakeCommandRunner{
		shellOutputs: make(map[string]string),
		shellErrors:  make(map[string]error),
		outputs:      make(map[string]string),
		errors:       make(map[string]error),
	}
}

func (f *fakeCommandRunner) GetOutput(command []string) (string, error) {
	f.outputCalls = append(f.outputCalls, command)
	key := fmt.Sprintf("%v", command)
	if err, ok := f.errors[key]; ok {
		return "", err
	}
	return f.outputs[key], nil
}

func (f *fakeCommandRunner) GetShellOutput(command string) (string, error) {
	if err, ok := f.shellErrors[command]; ok {
		return "", err
	}
	return f.shellOutputs[command], nil
}

func fakeEnv(values map[string]string) EnvGetter {
	return func(key string) string {
		return values[key]
	}
}

const kittyLsSingleOSWindow = `[
  {
    "id": 1,
    "tabs": [
      {
        "id": 10,
        "title": "workspace-1",
        "windows": [
          {"id": 100, "cwd": "/home/user/ws-1"}
        ]
      },
      {
        "id": 20,
        "title": "workspace-2",
        "windows": [
          {"id": 200, "cwd": "/home/user/ws-2"}
        ]
      },
      {
        "id": 30,
        "title": "workspace-3",
        "windows": [
          {"id": 300, "cwd": "/home/user/ws-3"}
        ]
      }
    ]
  }
]`

const kittyLsMultipleOSWindows = `[
  {
    "id": 1,
    "tabs": [
      {
        "id": 10,
        "title": "os1-tab",
        "windows": [
          {"id": 100, "cwd": "/home/user/os1"}
        ]
      }
    ]
  },
  {
    "id": 2,
    "tabs": [
      {
        "id": 20,
        "title": "os2-tab-a",
        "windows": [
          {"id": 200, "cwd": "/home/user/os2-a"}
        ]
      },
      {
        "id": 30,
        "title": "os2-tab-b",
        "windows": [
          {"id": 300, "cwd": "/home/user/os2-b"}
        ]
      }
    ]
  }
]`

const kittyLsMultipleWindows = `[
  {
    "id": 1,
    "tabs": [
      {
        "id": 10,
        "title": "multi-win-tab",
        "windows": [
          {"id": 100, "cwd": "/home/user/first-window"},
          {"id": 101, "cwd": "/home/user/second-window"}
        ]
      }
    ]
  }
]`

func TestKittyListSessions_SingleOSWindow(t *testing.T) {
	runner := newFakeCommandRunner()
	runner.shellOutputs["kitty @ ls"] = kittyLsSingleOSWindow

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{"KITTY_WINDOW_ID": "200"}),
	}

	sessions, err := backend.ListSessions()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(sessions) != 3 {
		t.Fatalf("expected 3 sessions, got %d", len(sessions))
	}

	expected := []Session{
		{ID: "10", Name: "workspace-1", RootPath: "/home/user/ws-1"},
		{ID: "20", Name: "workspace-2", RootPath: "/home/user/ws-2"},
		{ID: "30", Name: "workspace-3", RootPath: "/home/user/ws-3"},
	}
	for i, s := range sessions {
		if s.ID != expected[i].ID {
			t.Errorf("session %d: expected ID %q, got %q", i, expected[i].ID, s.ID)
		}
		if s.Name != expected[i].Name {
			t.Errorf("session %d: expected Name %q, got %q", i, expected[i].Name, s.Name)
		}
		if s.RootPath != expected[i].RootPath {
			t.Errorf("session %d: expected RootPath %q, got %q", i, expected[i].RootPath, s.RootPath)
		}
	}
}

func TestKittyListSessions_MultipleOSWindows(t *testing.T) {
	runner := newFakeCommandRunner()
	runner.shellOutputs["kitty @ ls"] = kittyLsMultipleOSWindows

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{"KITTY_WINDOW_ID": "300"}),
	}

	sessions, err := backend.ListSessions()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(sessions) != 2 {
		t.Fatalf("expected 2 sessions (from second OS window), got %d", len(sessions))
	}
	if sessions[0].Name != "os2-tab-a" {
		t.Errorf("expected first session name %q, got %q", "os2-tab-a", sessions[0].Name)
	}
	if sessions[1].Name != "os2-tab-b" {
		t.Errorf("expected second session name %q, got %q", "os2-tab-b", sessions[1].Name)
	}
}

func TestKittyListSessions_WindowIDNotFound(t *testing.T) {
	runner := newFakeCommandRunner()
	runner.shellOutputs["kitty @ ls"] = kittyLsSingleOSWindow

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{"KITTY_WINDOW_ID": "9999"}),
	}

	sessions, err := backend.ListSessions()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(sessions) != 0 {
		t.Fatalf("expected 0 sessions, got %d", len(sessions))
	}
}

func TestKittyListSessions_WindowIDNotSet(t *testing.T) {
	runner := newFakeCommandRunner()

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{}),
	}

	sessions, err := backend.ListSessions()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(sessions) != 0 {
		t.Fatalf("expected 0 sessions, got %d", len(sessions))
	}
}

func TestKittyListSessions_TabIDConvertedToString(t *testing.T) {
	runner := newFakeCommandRunner()
	runner.shellOutputs["kitty @ ls"] = kittyLsSingleOSWindow

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{"KITTY_WINDOW_ID": "100"}),
	}

	sessions, err := backend.ListSessions()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(sessions) == 0 {
		t.Fatal("expected sessions, got none")
	}
	// Tab IDs 10, 20, 30 should be string "10", "20", "30"
	if sessions[0].ID != "10" {
		t.Errorf("expected ID %q, got %q", "10", sessions[0].ID)
	}
}

func TestKittyListSessions_RootPathFromFirstWindow(t *testing.T) {
	runner := newFakeCommandRunner()
	runner.shellOutputs["kitty @ ls"] = kittyLsMultipleWindows

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{"KITTY_WINDOW_ID": "100"}),
	}

	sessions, err := backend.ListSessions()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(sessions) != 1 {
		t.Fatalf("expected 1 session, got %d", len(sessions))
	}
	if sessions[0].RootPath != "/home/user/first-window" {
		t.Errorf("expected RootPath %q, got %q", "/home/user/first-window", sessions[0].RootPath)
	}
}

func TestKittyAttach(t *testing.T) {
	runner := newFakeCommandRunner()
	runner.shellOutputs["kitty @ focus-tab --match id:42"] = ""

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{}),
	}

	err := backend.Attach("42")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestKittyCreateAndAttach(t *testing.T) {
	runner := newFakeCommandRunner()

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{"SHELL": "/bin/fish"}),
	}

	err := backend.CreateAndAttach("my-ws", "/home/user/ws", "nvim")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(runner.outputCalls) != 1 {
		t.Fatalf("expected 1 GetOutput call, got %d", len(runner.outputCalls))
	}
	args := runner.outputCalls[0]
	expected := []string{
		"kitty", "@", "launch",
		"--type=tab",
		"--tab-title", "my-ws",
		"--cwd=/home/user/ws",
		"/bin/fish", "--login", "--interactive",
		"-c", "nvim",
	}
	if len(args) != len(expected) {
		t.Fatalf("expected %d args, got %d: %v", len(expected), len(args), args)
	}
	for i, arg := range args {
		if arg != expected[i] {
			t.Errorf("arg %d: expected %q, got %q", i, expected[i], arg)
		}
	}
}

func TestKittyCreateAndAttach_DefaultShell(t *testing.T) {
	runner := newFakeCommandRunner()

	backend := &KittyBackend{
		Runner: runner,
		GetEnv: fakeEnv(map[string]string{}),
	}

	err := backend.CreateAndAttach("my-ws", "/home/user/ws", "nvim")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(runner.outputCalls) != 1 {
		t.Fatalf("expected 1 GetOutput call, got %d", len(runner.outputCalls))
	}
	args := runner.outputCalls[0]
	// When SHELL is not set, should fall back to /bin/sh
	if args[7] != "/bin/sh" {
		t.Errorf("expected shell %q, got %q", "/bin/sh", args[7])
	}
}
