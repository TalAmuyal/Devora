package version

import (
	"os"
	"path/filepath"
	"strings"
)

// Get returns the effective version string.
// It reads from $DEVORA_RESOURCES_DIR/VERSION if available,
// otherwise returns "dev".
func Get() string {
	resourcesDir := os.Getenv("DEVORA_RESOURCES_DIR")
	if resourcesDir == "" {
		return "dev"
	}
	data, err := os.ReadFile(filepath.Join(resourcesDir, "VERSION"))
	if err != nil {
		return "dev"
	}
	v := strings.TrimSpace(string(data))
	if v == "" {
		return "dev"
	}
	return v
}
