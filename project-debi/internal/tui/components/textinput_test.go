package components

import (
	"testing"

	"charm.land/lipgloss/v2"
)

func newTestTextInput() TextInputModel {
	m := NewTextInputModel(lipgloss.NewStyle(), lipgloss.NewStyle())
	m.Focus()
	return m
}

func TestSpaceInsertsSpace(t *testing.T) {
	m := newTestTextInput()

	m.HandleKey("h")
	m.HandleKey("i")
	consumed := m.HandleKey("space")

	if !consumed {
		t.Fatal("expected space to be consumed")
	}
	if m.Value != "hi " {
		t.Fatalf("expected 'hi ', got %q", m.Value)
	}
}

func TestSpaceBetweenWords(t *testing.T) {
	m := newTestTextInput()

	m.HandleKey("a")
	m.HandleKey("space")
	m.HandleKey("b")

	if m.Value != "a b" {
		t.Fatalf("expected 'a b', got %q", m.Value)
	}
}

func TestTypingCharacters(t *testing.T) {
	m := newTestTextInput()

	m.HandleKey("a")
	m.HandleKey("b")
	m.HandleKey("c")

	if m.Value != "abc" {
		t.Fatalf("expected 'abc', got %q", m.Value)
	}
}

func TestBackspace(t *testing.T) {
	m := newTestTextInput()

	m.HandleKey("a")
	m.HandleKey("b")
	m.HandleKey("backspace")

	if m.Value != "a" {
		t.Fatalf("expected 'a', got %q", m.Value)
	}
}

func TestCursorMovement(t *testing.T) {
	m := newTestTextInput()

	m.HandleKey("a")
	m.HandleKey("b")
	m.HandleKey("c")
	m.HandleKey("left")
	m.HandleKey("left")
	m.HandleKey("x")

	if m.Value != "axbc" {
		t.Fatalf("expected 'axbc', got %q", m.Value)
	}
}

func TestUnfocusedIgnoresKeys(t *testing.T) {
	m := newTestTextInput()
	m.Blur()

	consumed := m.HandleKey("a")
	if consumed {
		t.Fatal("expected key to not be consumed when blurred")
	}
	if m.Value != "" {
		t.Fatalf("expected empty value, got %q", m.Value)
	}
}
