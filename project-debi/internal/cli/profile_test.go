package cli

import (
	"errors"
	"testing"

	"devora/internal/config"
)

// stubResolveActiveProfileDeps overrides the package-level stubbable vars
// used by ResolveActiveProfile, and also resets the active profile via
// config.ResetForTesting so tests don't leak state between runs.
func stubResolveActiveProfileDeps(
	t *testing.T,
	cwdFn func() (string, error),
	profilesFn func() []config.Profile,
	resolveFn func(cwd string) (*config.Profile, string, error),
) {
	t.Helper()
	origCWD := getCWD
	origProfiles := getProfiles
	origResolve := resolveFromCWD
	origEnv := getProfilePathEnv
	t.Cleanup(func() {
		getCWD = origCWD
		getProfiles = origProfiles
		resolveFromCWD = origResolve
		getProfilePathEnv = origEnv
		config.ResetForTesting()
	})
	getCWD = cwdFn
	getProfiles = profilesFn
	resolveFromCWD = resolveFn
	// Default: no DEBI_PROFILE_PATH.
	// Tests that exercise the env default override this var explicitly.
	getProfilePathEnv = func() string { return "" }
}

func TestResolveActiveProfile_ExplicitName_Found_SetsActiveAndReturnsName(t *testing.T) {
	target := config.Profile{Name: "work", RootPath: "/tmp/work", Config: map[string]any{"name": "work"}}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/nothing", nil },
		func() []config.Profile { return []config.Profile{{Name: "play"}, target} },
		func(cwd string) (*config.Profile, string, error) {
			t.Fatal("resolveFromCWD should not be called when explicit profile is given")
			return nil, "", nil
		},
	)

	name, err := ResolveActiveProfile("work")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected resolved name 'work', got %q", name)
	}
	active := config.GetActiveProfile()
	if active == nil {
		t.Fatal("expected active profile to be set")
	}
	if active.Name != "work" {
		t.Fatalf("expected active profile name 'work', got %q", active.Name)
	}
}

func TestResolveActiveProfile_ExplicitName_NotFound_ReturnsError(t *testing.T) {
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/nothing", nil },
		func() []config.Profile { return []config.Profile{{Name: "play"}} },
		func(cwd string) (*config.Profile, string, error) {
			t.Fatal("resolveFromCWD should not be called when explicit profile is given")
			return nil, "", nil
		},
	)

	name, err := ResolveActiveProfile("work")
	if err == nil {
		t.Fatal("expected error for unknown profile name")
	}
	if name != "" {
		t.Fatalf("expected empty name on error, got %q", name)
	}
	if config.GetActiveProfile() != nil {
		t.Fatal("expected active profile to remain nil on error")
	}
}

func TestResolveActiveProfile_EmptyExplicit_CWDMatch_SetsActive(t *testing.T) {
	target := config.Profile{Name: "work", RootPath: "/tmp/work"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/work/workspaces/ws-1", nil },
		func() []config.Profile { return []config.Profile{target} },
		func(cwd string) (*config.Profile, string, error) {
			if cwd != "/tmp/work/workspaces/ws-1" {
				t.Fatalf("resolveFromCWD called with unexpected cwd: %q", cwd)
			}
			return &target, "/tmp/work/workspaces/ws-1", nil
		},
	)

	name, err := ResolveActiveProfile("")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected resolved name 'work', got %q", name)
	}
	active := config.GetActiveProfile()
	if active == nil || active.Name != "work" {
		t.Fatalf("expected active profile 'work', got %+v", active)
	}
}

func TestResolveActiveProfile_EmptyExplicit_CWDNoMatch_LeavesActiveNil(t *testing.T) {
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/random", nil },
		func() []config.Profile { return []config.Profile{{Name: "play"}} },
		func(cwd string) (*config.Profile, string, error) { return nil, "", nil },
	)

	name, err := ResolveActiveProfile("")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "" {
		t.Fatalf("expected empty resolved name, got %q", name)
	}
	if config.GetActiveProfile() != nil {
		t.Fatal("expected active profile to remain nil")
	}
}

func TestResolveActiveProfile_ExplicitOverridesCWD(t *testing.T) {
	// Explicit wins: CWD resolver must NOT be consulted when explicit name is
	// provided, even if the CWD would also match.
	target := config.Profile{Name: "work"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/play/workspaces/ws-1", nil },
		func() []config.Profile {
			return []config.Profile{{Name: "play"}, target}
		},
		func(cwd string) (*config.Profile, string, error) {
			t.Fatal("resolveFromCWD should not be called when explicit profile is given")
			return nil, "", nil
		},
	)

	name, err := ResolveActiveProfile("work")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected resolved name 'work', got %q", name)
	}
	active := config.GetActiveProfile()
	if active == nil || active.Name != "work" {
		t.Fatalf("expected active profile 'work', got %+v", active)
	}
}

func TestResolveActiveProfile_CWDErrorTreatedAsNoMatch(t *testing.T) {
	// When os.Getwd fails, ResolveActiveProfile should fall back to the
	// "no match" case (nil active profile, no error). A transient CWD error
	// should not break the command.
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "", errors.New("getwd failed") },
		func() []config.Profile { return []config.Profile{{Name: "play"}} },
		func(cwd string) (*config.Profile, string, error) {
			t.Fatal("resolveFromCWD should not be called when getCWD failed")
			return nil, "", nil
		},
	)

	name, err := ResolveActiveProfile("")
	if err != nil {
		t.Fatalf("expected no error on CWD failure, got: %s", err.Error())
	}
	if name != "" {
		t.Fatalf("expected empty resolved name, got %q", name)
	}
	if config.GetActiveProfile() != nil {
		t.Fatal("expected active profile to remain nil")
	}
}

func TestResolveActiveProfileByPath_Found_SetsActiveAndReturnsName(t *testing.T) {
	target := config.Profile{Name: "work", RootPath: "/tmp/work"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { t.Fatal("getCWD should not be called when explicit path matches"); return "", nil },
		func() []config.Profile { return []config.Profile{{Name: "play", RootPath: "/tmp/play"}, target} },
		func(cwd string) (*config.Profile, string, error) {
			t.Fatal("resolveFromCWD should not be called when explicit path is given")
			return nil, "", nil
		},
	)

	name, err := ResolveActiveProfileByPath("/tmp/work")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected resolved name 'work', got %q", name)
	}
	active := config.GetActiveProfile()
	if active == nil || active.Name != "work" {
		t.Fatalf("expected active profile 'work', got %+v", active)
	}
}

func TestResolveActiveProfileByPath_NormalizesTrailingSlash(t *testing.T) {
	// A path with a trailing slash (or other un-cleaned form) still matches.
	target := config.Profile{Name: "work", RootPath: "/tmp/work"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/x", nil },
		func() []config.Profile { return []config.Profile{target} },
		func(cwd string) (*config.Profile, string, error) { return nil, "", nil },
	)

	name, err := ResolveActiveProfileByPath("/tmp/work/")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected resolved name 'work', got %q", name)
	}
}

func TestResolveActiveProfileByPath_NotFound_ReturnsError(t *testing.T) {
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/x", nil },
		func() []config.Profile { return []config.Profile{{Name: "play", RootPath: "/tmp/play"}} },
		func(cwd string) (*config.Profile, string, error) { return nil, "", nil },
	)

	name, err := ResolveActiveProfileByPath("/tmp/work")
	if err == nil {
		t.Fatal("expected error for unknown profile path")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if name != "" {
		t.Fatalf("expected empty name on error, got %q", name)
	}
	if config.GetActiveProfile() != nil {
		t.Fatal("expected active profile to remain nil on error")
	}
}

func TestResolveActiveProfile_EnvPath_Default_SetsActive(t *testing.T) {
	// DEBI_PROFILE_PATH acts as a default when no explicit flag is given.
	target := config.Profile{Name: "work", RootPath: "/tmp/work"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { t.Fatal("getCWD should not be called when env path matches"); return "", nil },
		func() []config.Profile { return []config.Profile{{Name: "play", RootPath: "/tmp/play"}, target} },
		func(cwd string) (*config.Profile, string, error) {
			t.Fatal("resolveFromCWD should not be called when env path matches")
			return nil, "", nil
		},
	)
	getProfilePathEnv = func() string { return "/tmp/work" }

	name, err := ResolveActiveProfile("")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected resolved name 'work', got %q", name)
	}
}

func TestResolveActiveProfile_ExplicitNameOverridesEnvPath(t *testing.T) {
	// An explicit flag wins over the env default (env-vars are defaults).
	work := config.Profile{Name: "work", RootPath: "/tmp/work"}
	play := config.Profile{Name: "play", RootPath: "/tmp/play"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/x", nil },
		func() []config.Profile { return []config.Profile{work, play} },
		func(cwd string) (*config.Profile, string, error) { return nil, "", nil },
	)
	getProfilePathEnv = func() string { return "/tmp/play" }

	name, err := ResolveActiveProfile("work")
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected explicit name 'work' to win over env path, got %q", name)
	}
}

func TestResolveActiveProfile_EnvPath_NoMatch_FallsThroughToCWD(t *testing.T) {
	// A stale env path (matching no profile) must NOT error — it falls through to CWD resolution so the command still works.
	target := config.Profile{Name: "work", RootPath: "/tmp/work"}
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/work/workspaces/ws-1", nil },
		func() []config.Profile { return []config.Profile{target} },
		func(cwd string) (*config.Profile, string, error) { return &target, cwd, nil },
	)
	getProfilePathEnv = func() string { return "/tmp/deleted-profile" }

	name, err := ResolveActiveProfile("")
	if err != nil {
		t.Fatalf("expected no error when env path is stale, got: %s", err.Error())
	}
	if name != "work" {
		t.Fatalf("expected fall-through to CWD match 'work', got %q", name)
	}
}

func TestResolveActiveProfile_ResolverError_BubblesUp(t *testing.T) {
	// If resolveFromCWD itself returns a real error (not just "no match"), that error is propagated so the caller can surface it.
	stubResolveActiveProfileDeps(
		t,
		func() (string, error) { return "/tmp/random", nil },
		func() []config.Profile { return []config.Profile{{Name: "play"}} },
		func(cwd string) (*config.Profile, string, error) {
			return nil, "", errors.New("resolver boom")
		},
	)

	name, err := ResolveActiveProfile("")
	if err == nil {
		t.Fatal("expected error from resolveFromCWD to propagate")
	}
	if name != "" {
		t.Fatalf("expected empty resolved name, got %q", name)
	}
	if config.GetActiveProfile() != nil {
		t.Fatal("expected active profile to remain nil on resolver error")
	}
}
