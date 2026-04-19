package terminal

import (
	"encoding/json"
	"fmt"
	"strconv"
)

type KittyBackend struct {
	Runner CommandRunner
	GetEnv EnvGetter
}

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

func findTabsForWindow(osWindows []kittyOSWindow, kittyWindowID int) []kittyTab {
	for _, osWin := range osWindows {
		for _, tab := range osWin.Tabs {
			for _, win := range tab.Windows {
				if win.ID == kittyWindowID {
					return osWin.Tabs
				}
			}
		}
	}
	return nil
}

func (k *KittyBackend) ListSessions() ([]Session, error) {
	windowIDStr := k.GetEnv("KITTY_WINDOW_ID")
	if windowIDStr == "" {
		return []Session{}, nil
	}

	kittyWindowID, err := strconv.Atoi(windowIDStr)
	if err != nil {
		return nil, fmt.Errorf("invalid KITTY_WINDOW_ID %q: %w", windowIDStr, err)
	}

	output, err := k.Runner.GetShellOutput("kitty @ ls")
	if err != nil {
		return nil, err
	}

	var osWindows []kittyOSWindow
	if err := json.Unmarshal([]byte(output), &osWindows); err != nil {
		return nil, fmt.Errorf("failed to parse kitty ls output: %w", err)
	}

	tabs := findTabsForWindow(osWindows, kittyWindowID)
	if tabs == nil {
		return []Session{}, nil
	}

	sessions := make([]Session, 0, len(tabs))
	for _, tab := range tabs {
		if len(tab.Windows) == 0 {
			continue
		}
		sessions = append(sessions, Session{
			ID:       strconv.Itoa(tab.ID),
			Name:     tab.Title,
			RootPath: tab.Windows[0].CWD,
		})
	}

	return sessions, nil
}

func (k *KittyBackend) Attach(sessionID string) error {
	_, err := k.Runner.GetShellOutput(fmt.Sprintf("kitty @ focus-tab --match id:%s", sessionID))
	return err
}

func (k *KittyBackend) CreateAndAttach(sessionName, workingDirectory, app string) error {
	shell := k.GetEnv("SHELL")
	if shell == "" {
		shell = "/bin/sh"
	}
	args := []string{
		"kitty", "@", "launch",
		"--type=tab",
		"--tab-title", sessionName,
		fmt.Sprintf("--cwd=%s", workingDirectory),
		shell, "-l", "-i",
	}
	// Skip the `-c <app>` wrapping if the requested app is already the shell,
	// either via the explicit "shell" sentinel or by matching $SHELL directly.
	if app != "shell" && app != shell {
		args = append(args, "-c", app)
	}
	_, err := k.Runner.GetOutput(args)
	return err
}
