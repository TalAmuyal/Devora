package cli

import (
	"fmt"
	"os"

	"devora/internal/config"
	"devora/internal/workspace"
)

// Package-level stubbable vars so tests can exercise ResolveActiveProfile
// without touching the filesystem or real config.
var (
	resolveFromCWD = workspace.ResolveWorkspaceFromCWD
	getProfiles    = config.GetProfiles
	getCWD         = os.Getwd
)

// ResolveActiveProfile picks the active profile for a command and registers
// it via config.SetActiveProfile. Resolution order, stopping at the first hit:
//
//  1. explicitProfile != "" — look up by name in config.GetProfiles().
//     When the name matches no registered profile, returns a UsageError
//     without touching the active-profile state.
//  2. CWD-based — workspace.ResolveWorkspaceFromCWD(cwd). If the CWD falls
//     inside a profile's workspaces root, that profile becomes active.
//  3. Leave config.activeProfile nil so callers fall back to global config.
//
// Returns the resolved profile name on success (empty string for case 3).
//
// A failure from os.Getwd is treated as "no CWD match" (case 3 fallback),
// because an unreadable CWD is not a fatal error for a command that can
// still run against global config.
func ResolveActiveProfile(explicitProfile string) (string, error) {
	if explicitProfile != "" {
		profiles := getProfiles()
		for i := range profiles {
			profile := profiles[i]
			if profile.Name == explicitProfile {
				config.SetActiveProfile(&profile)
				return profile.Name, nil
			}
		}
		return "", &UsageError{Message: fmt.Sprintf("profile not found: %s", explicitProfile)}
	}

	cwd, err := getCWD()
	if err != nil {
		// Unreadable CWD — fall back to global config.
		return "", nil
	}

	profile, _, err := resolveFromCWD(cwd)
	if err != nil {
		return "", err
	}
	if profile == nil {
		return "", nil
	}

	config.SetActiveProfile(profile)
	return profile.Name, nil
}
