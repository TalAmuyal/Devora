package tui

import (
	"testing"

	tea "charm.land/bubbletea/v2"

	"devora/internal/style"
)

func TestDeleteConfirm_Q_CancelsAndGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewDeleteConfirmModel(&styles)
	m.SetWorkspace(&WorkspaceInfo{
		Path: "/tmp/ws-1",
		Name: "ws-1",
	})

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'q'})
	cmd := m.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a command from q key, got nil")
	}

	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestDeleteConfirm_N_CancelsAndGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewDeleteConfirmModel(&styles)
	m.SetWorkspace(&WorkspaceInfo{
		Path: "/tmp/ws-1",
		Name: "ws-1",
	})

	keyMsg := tea.KeyPressMsg(tea.Key{Code: 'n'})
	cmd := m.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a command from n key, got nil")
	}

	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}

func TestDeleteConfirm_Escape_CancelsAndGoesBack(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewDeleteConfirmModel(&styles)
	m.SetWorkspace(&WorkspaceInfo{
		Path: "/tmp/ws-1",
		Name: "ws-1",
	})

	keyMsg := tea.KeyPressMsg(tea.Key{Code: tea.KeyEscape})
	cmd := m.Update(keyMsg)

	if cmd == nil {
		t.Fatal("expected a command from escape key, got nil")
	}

	msg := cmd()
	if _, ok := msg.(showWorkspaceListMsg); !ok {
		t.Fatalf("expected showWorkspaceListMsg, got %T", msg)
	}
}
