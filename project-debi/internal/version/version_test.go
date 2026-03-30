package version

import (
	"os"
	"path/filepath"
	"testing"
)

func TestGet_ReturnsDevWhenEnvUnset(t *testing.T) {
	t.Setenv("DEVORA_RESOURCES_DIR", "")
	os.Unsetenv("DEVORA_RESOURCES_DIR")

	result := Get()

	if result != "dev" {
		t.Errorf("expected 'dev', got %q", result)
	}
}

func TestGet_ReturnsDevWhenFileNotFound(t *testing.T) {
	t.Setenv("DEVORA_RESOURCES_DIR", t.TempDir())

	result := Get()

	if result != "dev" {
		t.Errorf("expected 'dev', got %q", result)
	}
}

func TestGet_ReturnsVersionFromFile(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "VERSION"), []byte("2026-03-28.0"), 0644)
	t.Setenv("DEVORA_RESOURCES_DIR", dir)

	result := Get()

	if result != "2026-03-28.0" {
		t.Errorf("expected '2026-03-28.0', got %q", result)
	}
}

func TestGet_TrimsWhitespace(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "VERSION"), []byte("2026-03-28.0\n"), 0644)
	t.Setenv("DEVORA_RESOURCES_DIR", dir)

	result := Get()

	if result != "2026-03-28.0" {
		t.Errorf("expected '2026-03-28.0', got %q", result)
	}
}

func TestGet_ReturnsDevForEmptyFile(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "VERSION"), []byte(""), 0644)
	t.Setenv("DEVORA_RESOURCES_DIR", dir)

	result := Get()

	if result != "dev" {
		t.Errorf("expected 'dev', got %q", result)
	}
}

func TestGet_ReturnsNightlyVersion(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "VERSION"), []byte("2026-03-28.0-dev.5"), 0644)
	t.Setenv("DEVORA_RESOURCES_DIR", dir)

	result := Get()

	if result != "2026-03-28.0-dev.5" {
		t.Errorf("expected '2026-03-28.0-dev.5', got %q", result)
	}
}
