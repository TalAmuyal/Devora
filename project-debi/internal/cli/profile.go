package cli

import (
	"fmt"
	"os"
	"path/filepath"

	"devora/internal/config"
	"devora/internal/workspace"
)

// Package-level stubbable vars so tests can exercise the profile resolver without touching the filesystem or real config.
var (
	resolveFromCWD    = workspace.ResolveWorkspaceFromCWD
	getProfiles       = config.GetProfiles
	getCWD            = os.Getwd
	getProfilePathEnv = func() string { return os.Getenv("DEBI_PROFILE_PATH") }
)

// ResolveActiveProfile picks the active profile by name and registers it via config.SetActiveProfile.
// See resolveActiveProfile for the resolution order.
//
// Returns the resolved profile name on success (empty string when nothing matches and the command should fall back to global config).
func ResolveActiveProfile(explicitProfile string) (string, error) {
	return resolveActiveProfileCore("", explicitProfile)
}

// ResolveActiveProfileByPath picks the active profile by an explicit root path (e.g. Devora's `debi health --profile-path <path>`), then the same env/CWD/ global fallbacks.
// Selecting by path avoids the ambiguity of profile names, which are not required to be unique.
func ResolveActiveProfileByPath(explicitPath string) (string, error) {
	return resolveActiveProfileCore(explicitPath, "")
}

// resolveActiveProfile resolves the active profile, stopping at the first hit:
//
//  1. explicitPath != "" — match a registered profile by root path.
//     No match is a UsageError (the caller asked for a specific profile).
//  2. explicitName != "" — match by name. No match is a UsageError.
//  3. DEBI_PROFILE_PATH env — a soft default (Devora sets it on session PTYs).
//     A path that matches no profile falls through rather than erroring, so a stale env var never breaks an otherwise-valid command.
//  4. CWD-based — workspace.ResolveWorkspaceFromCWD(cwd).
//  5. Leave config.activeProfile nil so callers fall back to global config.
//
// A failure from os.Getwd is treated as "no CWD match" (case 5 fallback), because an unreadable CWD is not a fatal error for a command that can still run against global config.
func resolveActiveProfileCore(explicitPath, explicitName string) (string, error) {
	if explicitPath != "" {
		profile := findProfileByPath(explicitPath)
		if profile == nil {
			return "", &UsageError{Message: fmt.Sprintf("profile not found at path: %s", explicitPath)}
		}
		config.SetActiveProfile(profile)
		return profile.Name, nil
	}

	if explicitName != "" {
		profiles := getProfiles()
		for i := range profiles {
			if profiles[i].Name == explicitName {
				config.SetActiveProfile(&profiles[i])
				return profiles[i].Name, nil
			}
		}
		return "", &UsageError{Message: fmt.Sprintf("profile not found: %s", explicitName)}
	}

	if envPath := getProfilePathEnv(); envPath != "" {
		if profile := findProfileByPath(envPath); profile != nil {
			config.SetActiveProfile(profile)
			return profile.Name, nil
		}
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

// findProfileByPath returns the registered profile whose root path matches path (compared after filepath.Clean), or nil when none matches.
func findProfileByPath(path string) *config.Profile {
	target := filepath.Clean(path)
	profiles := getProfiles()
	for i := range profiles {
		if filepath.Clean(profiles[i].RootPath) == target {
			return &profiles[i]
		}
	}
	return nil
}
