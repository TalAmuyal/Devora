package components

import (
	"strings"
	"testing"

	"charm.land/lipgloss/v2"
)

func TestRenderFooter_EmptyBindings_ReturnsEmpty(t *testing.T) {
	result := RenderFooter(nil, lipgloss.NewStyle(), lipgloss.NewStyle(), lipgloss.NewStyle(), 80, "")

	if result != "" {
		t.Fatalf("expected empty string for no bindings, got %q", result)
	}
}

func TestRenderFooter_WithBindings_RendersKeybindings(t *testing.T) {
	bindings := []KeyBinding{
		{Key: "enter", Desc: "Select"},
		{Key: "q", Desc: "Quit"},
	}

	result := RenderFooter(bindings, lipgloss.NewStyle(), lipgloss.NewStyle(), lipgloss.NewStyle(), 80, "")

	if !strings.Contains(result, "enter") {
		t.Fatal("expected result to contain 'enter'")
	}
	if !strings.Contains(result, "Select") {
		t.Fatal("expected result to contain 'Select'")
	}
	if !strings.Contains(result, "q") {
		t.Fatal("expected result to contain 'q'")
	}
	if !strings.Contains(result, "Quit") {
		t.Fatal("expected result to contain 'Quit'")
	}
}

func TestRenderFooter_WithVersionText_IncludesVersion(t *testing.T) {
	bindings := []KeyBinding{
		{Key: "q", Desc: "Quit"},
	}

	result := RenderFooter(bindings, lipgloss.NewStyle(), lipgloss.NewStyle(), lipgloss.NewStyle(), 80, "v1.2.3")

	if !strings.Contains(result, "v1.2.3") {
		t.Fatalf("expected result to contain version text 'v1.2.3', got %q", result)
	}
}

func TestRenderFooter_WithEmptyVersionText_DoesNotAddVersion(t *testing.T) {
	bindings := []KeyBinding{
		{Key: "q", Desc: "Quit"},
	}

	resultWithVersion := RenderFooter(bindings, lipgloss.NewStyle(), lipgloss.NewStyle(), lipgloss.NewStyle(), 80, "v1.2.3")
	resultWithoutVersion := RenderFooter(bindings, lipgloss.NewStyle(), lipgloss.NewStyle(), lipgloss.NewStyle(), 80, "")

	if len(resultWithoutVersion) >= len(resultWithVersion) {
		t.Fatal("expected result without version to be shorter than result with version")
	}
}

func TestRenderFooter_VersionTextNarrowWidth_StillRenders(t *testing.T) {
	bindings := []KeyBinding{
		{Key: "q", Desc: "Quit"},
	}

	// Very narrow width - version should still appear with a minimum gap of 2
	result := RenderFooter(bindings, lipgloss.NewStyle(), lipgloss.NewStyle(), lipgloss.NewStyle(), 10, "v1.2.3")

	if !strings.Contains(result, "v1.2.3") {
		t.Fatalf("expected result to contain version text even at narrow width, got %q", result)
	}
}
