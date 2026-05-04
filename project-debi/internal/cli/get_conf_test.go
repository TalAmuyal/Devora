package cli

import (
	"devora/internal/process"
	"errors"
	"strings"
	"testing"
)

// stubConfigGet overrides configGet for the duration of the test.
func stubConfigGet(t *testing.T, fn func(path string) (any, bool)) {
	t.Helper()
	orig := configGet
	configGet = fn
	t.Cleanup(func() { configGet = orig })
}

func TestRun_GetConf_NoArgs_ReturnsUsageError(t *testing.T) {
	err := runGetConf([]string{})
	if err == nil {
		t.Fatal("expected error for get-conf without args")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
}

func TestRun_GetConf_StringValue_PrintsRaw(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubConfigGet(t, func(path string) (any, bool) {
		return "overlay", true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"terminal.default-app"})
	out := stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if out != "overlay\n" {
		t.Fatalf("expected 'overlay\\n', got %q", out)
	}
}

func TestRun_GetConf_BoolValue_PrintsLiteral(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubConfigGet(t, func(path string) (any, bool) {
		return true, true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"pr.auto-merge"})
	out := stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if out != "true\n" {
		t.Fatalf("expected 'true\\n', got %q", out)
	}
}

func TestRun_GetConf_NumericValue_PrintsInteger(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubConfigGet(t, func(path string) (any, bool) {
		return float64(42), true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"terminal.session-creation-timeout-seconds"})
	out := stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if out != "42\n" {
		t.Fatalf("expected '42\\n', got %q", out)
	}
}

func TestRun_GetConf_MapValue_PrintsJSON(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubConfigGet(t, func(path string) (any, bool) {
		return map[string]any{"a": "b"}, true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"task-tracker"})
	out := stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if out != "{\"a\":\"b\"}\n" {
		t.Fatalf("expected '{\"a\":\"b\"}\\n', got %q", out)
	}
}

func TestRun_GetConf_KeyNotFound_ExitsNonZero(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubConfigGet(t, func(path string) (any, bool) {
		return nil, false
	})

	err := runGetConf([]string{"nonexistent"})
	if err == nil {
		t.Fatal("expected error for not-found key")
	}
	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", err, err.Error())
	}
	if ptErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", ptErr.Code)
	}
}

func TestRun_GetConf_UnknownFlag_ReturnsUsageError(t *testing.T) {
	err := runGetConf([]string{"--foo", "key"})
	if err == nil {
		t.Fatal("expected error for unknown flag")
	}
	var usageErr *UsageError
	if !errors.As(err, &usageErr) {
		t.Fatalf("expected UsageError, got %T: %s", err, err.Error())
	}
	if !strings.Contains(err.Error(), "--foo") {
		t.Fatalf("expected error to mention the flag, got: %s", err.Error())
	}
}

func TestRun_GetConf_ProfileFlag_PassedToResolver(t *testing.T) {
	captured := ""
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		captured = explicit
		return explicit, nil
	})
	stubConfigGet(t, func(path string) (any, bool) {
		return "value", true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"--profile", "work", "some.key"})
	_ = stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "work" {
		t.Fatalf("expected resolver called with 'work', got %q", captured)
	}
}

func TestRun_GetConf_ProfileShortFlag_PassedToResolver(t *testing.T) {
	captured := ""
	stubResolveActiveProfile(t, func(explicit string) (string, error) {
		captured = explicit
		return explicit, nil
	})
	stubConfigGet(t, func(path string) (any, bool) {
		return "value", true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"-p", "work", "some.key"})
	_ = stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if captured != "work" {
		t.Fatalf("expected resolver called with 'work', got %q", captured)
	}
}

func TestRun_GetConf_Help_PrintsUsage(t *testing.T) {
	stop := captureStdout(t)
	err := runGetConf([]string{"--help"})
	out := stop()

	if err != nil {
		t.Fatalf("expected no error for --help, got: %s", err.Error())
	}
	if !strings.Contains(out, "usage: debi get-conf") {
		t.Fatalf("expected usage message on stdout, got: %q", out)
	}
}

func TestRun_GetConf_FloatWithFraction_PrintsDecimal(t *testing.T) {
	stubResolveActiveProfile(t, func(string) (string, error) { return "", nil })
	stubConfigGet(t, func(path string) (any, bool) {
		return float64(3.14), true
	})

	stop := captureStdout(t)
	err := runGetConf([]string{"some.float"})
	out := stop()

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	if out != "3.14\n" {
		t.Fatalf("expected '3.14\\n', got %q", out)
	}
}
