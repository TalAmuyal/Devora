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
	t.Cleanup(func() {
		getCWD = origCWD
		getProfiles = origProfiles
		resolveFromCWD = origResolve
		config.ResetForTesting()
	})
	getCWD = cwdFn
	getProfiles = profilesFn
	resolveFromCWD = resolveFn
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

func TestResolveActiveProfile_ResolverError_BubblesUp(t *testing.T) {
	// If resolveFromCWD itself returns a real error (not just "no match"),
	// that error is propagated so the caller can surface it.
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
