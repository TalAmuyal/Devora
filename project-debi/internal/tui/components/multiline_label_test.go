package components

import (
	"testing"
)

func TestMultiLineLabelView_JoinsLines(t *testing.T) {
	label := NewMultiLineLabelModel(
		"First line",
		"Second line",
		"Third line",
	)

	got := label.View()
	expected := "First line\nSecond line\nThird line"

	if got != expected {
		t.Fatalf("expected %q, got %q", expected, got)
	}
}

func TestMultiLineLabelView_SingleLine(t *testing.T) {
	label := NewMultiLineLabelModel("Only line")

	got := label.View()
	if got != "Only line" {
		t.Fatalf("expected %q, got %q", "Only line", got)
	}
}

func TestMultiLineLabelView_Empty(t *testing.T) {
	label := NewMultiLineLabelModel()

	got := label.View()
	if got != "" {
		t.Fatalf("expected empty string, got %q", got)
	}
}
