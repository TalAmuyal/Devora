package components

import (
	"testing"

	"charm.land/lipgloss/v2"
)

func newTestCheckboxList(items ...string) CheckboxListModel {
	m := NewCheckboxListModel(lipgloss.NewStyle(), lipgloss.NewStyle())
	m.Items = items
	return m
}

func TestToggleWithSpace(t *testing.T) {
	m := newTestCheckboxList("alpha", "beta", "gamma")

	consumed := m.HandleKey("space")
	if !consumed {
		t.Fatal("expected space to be consumed")
	}
	if !m.Selected[0] {
		t.Fatal("expected item 0 to be selected after space")
	}

	// Toggle off
	consumed = m.HandleKey("space")
	if !consumed {
		t.Fatal("expected space to be consumed on second press")
	}
	if m.Selected[0] {
		t.Fatal("expected item 0 to be deselected after second space")
	}
}

func TestToggleAtDifferentCursors(t *testing.T) {
	m := newTestCheckboxList("alpha", "beta", "gamma")

	m.HandleKey("space") // select alpha
	m.CursorDown()
	m.HandleKey("space") // select beta

	selected := m.SelectedItems()
	if len(selected) != 2 {
		t.Fatalf("expected 2 selected items, got %d", len(selected))
	}
	if selected[0] != "alpha" || selected[1] != "beta" {
		t.Fatalf("expected [alpha, beta], got %v", selected)
	}
}

func TestToggleEmptyList(t *testing.T) {
	m := newTestCheckboxList()

	consumed := m.HandleKey("space")
	if !consumed {
		t.Fatal("expected space to be consumed even on empty list")
	}
	if len(m.SelectedItems()) != 0 {
		t.Fatal("expected no selected items on empty list")
	}
}

func TestCursorWrapping(t *testing.T) {
	m := newTestCheckboxList("a", "b", "c")

	// Wrap forward
	m.CursorDown()
	m.CursorDown()
	m.CursorDown()
	if m.Cursor != 0 {
		t.Fatalf("expected cursor to wrap to 0, got %d", m.Cursor)
	}

	// Wrap backward
	m.CursorUp()
	if m.Cursor != 2 {
		t.Fatalf("expected cursor to wrap to 2, got %d", m.Cursor)
	}
}

func TestVimNavigation(t *testing.T) {
	m := newTestCheckboxList("a", "b", "c")

	m.HandleKey("j") // down
	if m.Cursor != 1 {
		t.Fatalf("expected cursor 1 after j, got %d", m.Cursor)
	}

	m.HandleKey("k") // up
	if m.Cursor != 0 {
		t.Fatalf("expected cursor 0 after k, got %d", m.Cursor)
	}

	m.HandleKey("G") // go last
	if m.Cursor != 2 {
		t.Fatalf("expected cursor 2 after G, got %d", m.Cursor)
	}

	m.HandleKey("g") // first g
	m.HandleKey("g") // gg -> go first
	if m.Cursor != 0 {
		t.Fatalf("expected cursor 0 after gg, got %d", m.Cursor)
	}
}

func TestSelectedItemsOrder(t *testing.T) {
	m := newTestCheckboxList("alpha", "beta", "gamma")

	// Select in reverse order: gamma, then alpha
	m.Cursor = 2
	m.HandleKey("space")
	m.Cursor = 0
	m.HandleKey("space")

	selected := m.SelectedItems()
	if len(selected) != 2 {
		t.Fatalf("expected 2 selected, got %d", len(selected))
	}
	// SelectedItems returns in list order, not selection order
	if selected[0] != "alpha" || selected[1] != "gamma" {
		t.Fatalf("expected [alpha, gamma], got %v", selected)
	}
}

func TestUnknownKeyNotConsumed(t *testing.T) {
	m := newTestCheckboxList("a", "b")

	consumed := m.HandleKey("x")
	if consumed {
		t.Fatal("expected unknown key to not be consumed")
	}
}
