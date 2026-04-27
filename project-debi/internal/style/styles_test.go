package style

import (
	"strings"
	"testing"
)

// Each hue style must emit a 24-bit foreground SGR sequence ("38;2;<r>;<g>;<b>")
// matching its Mocha hex value. Asserting on the SGR substring (not the full
// escape) keeps the tests robust to lipgloss reset/wrapping changes while
// still pinning the exact RGB triple.

func TestGreenRendersMochaGreen(t *testing.T) {
	got := Green.Render("ok")
	want := "38;2;166;227;161"
	if !strings.Contains(got, want) {
		t.Fatalf("Green.Render: missing %q in %q", want, got)
	}
}

func TestRedRendersMochaRed(t *testing.T) {
	got := Red.Render("ok")
	want := "38;2;243;139;168"
	if !strings.Contains(got, want) {
		t.Fatalf("Red.Render: missing %q in %q", want, got)
	}
}

func TestYellowRendersMochaYellow(t *testing.T) {
	got := Yellow.Render("ok")
	want := "38;2;249;226;175"
	if !strings.Contains(got, want) {
		t.Fatalf("Yellow.Render: missing %q in %q", want, got)
	}
}

func TestCyanRendersMochaSky(t *testing.T) {
	got := Cyan.Render("ok")
	want := "38;2;137;220;235"
	if !strings.Contains(got, want) {
		t.Fatalf("Cyan.Render: missing %q in %q", want, got)
	}
}

func TestMutedRendersMochaOverlay0(t *testing.T) {
	got := Muted.Render("ok")
	want := "38;2;108;112;134"
	if !strings.Contains(got, want) {
		t.Fatalf("Muted.Render: missing %q in %q", want, got)
	}
}

// Alias-pin tests: each semantic alias must render identically to its hue
// counterpart. If anyone later remaps a semantic alias to a different hue,
// these tests will fail loudly so the change is conscious.

func TestSuccessAliasesGreen(t *testing.T) {
	if Success.Render("x") != Green.Render("x") {
		t.Fatalf("Success must alias Green")
	}
}

func TestErrorAliasesRed(t *testing.T) {
	if Error.Render("x") != Red.Render("x") {
		t.Fatalf("Error must alias Red")
	}
}

func TestWarningAliasesYellow(t *testing.T) {
	if Warning.Render("x") != Yellow.Render("x") {
		t.Fatalf("Warning must alias Yellow")
	}
}

func TestInfoAliasesCyan(t *testing.T) {
	if Info.Render("x") != Cyan.Render("x") {
		t.Fatalf("Info must alias Cyan")
	}
}
