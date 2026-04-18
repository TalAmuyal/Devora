package credentials

import (
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/zalando/go-keyring"
)

// stubCredentialsDeps captures and restores the overridable package-level
// vars used by GetToken so tests can stub them without leaking state.
func stubCredentialsDeps(t *testing.T) {
	t.Helper()
	origKeyringGet := keyringGet
	origGetUser := getUser
	origTimeout := timeout
	t.Cleanup(func() {
		keyringGet = origKeyringGet
		getUser = origGetUser
		timeout = origTimeout
	})
}

// --- GetToken: happy path ---

func TestGetToken_HappyPath(t *testing.T) {
	stubCredentialsDeps(t)

	getUser = func() (string, error) { return "alice", nil }
	keyringGet = func(service, user string) (string, error) {
		if service != "devora-asana" {
			t.Fatalf("unexpected service: %q", service)
		}
		if user != "alice" {
			t.Fatalf("unexpected user: %q", user)
		}
		return "tok123", nil
	}

	got, err := GetToken("asana")
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if got != "tok123" {
		t.Fatalf("expected token %q, got %q", "tok123", got)
	}
}

// --- GetToken: not-found -> *NotFoundError with proper fields ---

func TestGetToken_NotFound_ReturnsNotFoundError(t *testing.T) {
	stubCredentialsDeps(t)

	getUser = func() (string, error) { return "alice", nil }
	keyringGet = func(service, user string) (string, error) {
		return "", keyring.ErrNotFound
	}

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected an error, got nil")
	}

	var nfe *NotFoundError
	if !errors.As(err, &nfe) {
		t.Fatalf("expected *NotFoundError, got %T: %v", err, err)
	}
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("expected errors.Is(err, ErrNotFound) to be true, got false")
	}
	if nfe.Provider != "asana" {
		t.Errorf("Provider: expected %q, got %q", "asana", nfe.Provider)
	}
	if nfe.Service != "devora-asana" {
		t.Errorf("Service: expected %q, got %q", "devora-asana", nfe.Service)
	}
	if nfe.Account != "alice" {
		t.Errorf("Account: expected %q, got %q", "alice", nfe.Account)
	}
}

// --- GetToken: empty token treated as not-found ---

func TestGetToken_EmptyToken_ReturnsNotFoundError(t *testing.T) {
	stubCredentialsDeps(t)

	getUser = func() (string, error) { return "alice", nil }
	keyringGet = func(service, user string) (string, error) {
		return "", nil
	}

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	var nfe *NotFoundError
	if !errors.As(err, &nfe) {
		t.Fatalf("expected *NotFoundError, got %T: %v", err, err)
	}
}

// --- GetToken: unsupported platform error is wrapped with a clear message ---

func TestGetToken_UnsupportedPlatform(t *testing.T) {
	stubCredentialsDeps(t)

	getUser = func() (string, error) { return "alice", nil }
	keyringGet = func(service, user string) (string, error) {
		return "", keyring.ErrUnsupportedPlatform
	}

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	if !errors.Is(err, keyring.ErrUnsupportedPlatform) {
		t.Fatalf("expected errors.Is(err, keyring.ErrUnsupportedPlatform) to be true, got: %v", err)
	}
	msg := err.Error()
	if !strings.Contains(msg, "keychain") {
		t.Errorf("expected error to mention 'keychain', got: %q", msg)
	}
}

// --- GetToken: other keychain errors are wrapped ---

func TestGetToken_GenericKeyringError_IsWrapped(t *testing.T) {
	stubCredentialsDeps(t)

	sentinel := errors.New("dbus exploded")
	getUser = func() (string, error) { return "alice", nil }
	keyringGet = func(service, user string) (string, error) {
		return "", sentinel
	}

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	if !errors.Is(err, sentinel) {
		t.Fatalf("expected wrapped sentinel, got: %v", err)
	}
}

// --- GetToken: timeout path ---

func TestGetToken_Timeout(t *testing.T) {
	stubCredentialsDeps(t)

	getUser = func() (string, error) { return "alice", nil }
	// Block long enough to guarantee the shortened timeout fires.
	release := make(chan struct{})
	t.Cleanup(func() { close(release) })
	keyringGet = func(service, user string) (string, error) {
		<-release
		return "", nil
	}
	timeout = 20 * time.Millisecond

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected a timeout error, got nil")
	}
	if !strings.Contains(err.Error(), "timed out") {
		t.Fatalf("expected error message to contain 'timed out', got: %q", err.Error())
	}
}

// --- GetToken: getUser provides the account when $USER is empty ---

func TestGetToken_GetUserFallthrough(t *testing.T) {
	stubCredentialsDeps(t)

	getUser = func() (string, error) { return "actualuser", nil }
	keyringGet = func(service, user string) (string, error) {
		if user != "actualuser" {
			t.Fatalf("expected account %q, got %q", "actualuser", user)
		}
		return "", keyring.ErrNotFound
	}

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	var nfe *NotFoundError
	if !errors.As(err, &nfe) {
		t.Fatalf("expected *NotFoundError, got %T", err)
	}
	if nfe.Account != "actualuser" {
		t.Errorf("Account: expected %q, got %q", "actualuser", nfe.Account)
	}
}

// --- GetToken: getUser error is propagated ---

func TestGetToken_GetUserError_IsPropagated(t *testing.T) {
	stubCredentialsDeps(t)

	sentinel := errors.New("no user for you")
	getUser = func() (string, error) { return "", sentinel }
	keyringGet = func(service, user string) (string, error) {
		t.Fatal("keyringGet should not be called when getUser fails")
		return "", nil
	}

	_, err := GetToken("asana")
	if err == nil {
		t.Fatal("expected an error, got nil")
	}
	if !errors.Is(err, sentinel) {
		t.Fatalf("expected wrapped sentinel, got: %v", err)
	}
}

// --- defaultGetUser: covers $USER-empty fallthrough to os/user.Current ---

func TestDefaultGetUser_UsesUSEREnv(t *testing.T) {
	t.Setenv("USER", "from-env")
	got, err := defaultGetUser()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if got != "from-env" {
		t.Fatalf("expected %q, got %q", "from-env", got)
	}
}

func TestDefaultGetUser_EmptyUSER_FallsThroughToOSUser(t *testing.T) {
	t.Setenv("USER", "")
	got, err := defaultGetUser()
	// On any supported dev environment os/user.Current() succeeds, so we
	// only assert that we get a non-empty username. If the environment is
	// pathological, err will be non-nil and the assertion still holds by
	// failing the test with diagnostic output.
	if err != nil {
		t.Fatalf("expected os/user.Current fallthrough to succeed, got: %v", err)
	}
	if got == "" {
		t.Fatalf("expected non-empty username from os/user.Current, got empty")
	}
}

// --- NotFoundError: Error() format + Unwrap() ---

func TestNotFoundError_Error_ShortSingleLine(t *testing.T) {
	nfe := &NotFoundError{
		Provider: "asana",
		Service:  "devora-asana",
		Account:  "alice",
	}
	msg := nfe.Error()
	if strings.Contains(msg, "\n") {
		t.Fatalf("expected single-line message, got:\n%s", msg)
	}
	if !strings.Contains(msg, "devora-asana") {
		t.Errorf("expected message to contain service %q, got: %q", "devora-asana", msg)
	}
	if !strings.Contains(msg, "alice") {
		t.Errorf("expected message to contain account %q, got: %q", "alice", msg)
	}
}

func TestNotFoundError_Unwrap_ReturnsSentinel(t *testing.T) {
	nfe := &NotFoundError{Provider: "asana", Service: "devora-asana", Account: "alice"}
	if !errors.Is(nfe, ErrNotFound) {
		t.Fatalf("expected errors.Is(nfe, ErrNotFound) to be true")
	}
	if nfe.Unwrap() != ErrNotFound {
		t.Fatalf("expected Unwrap() to return ErrNotFound, got: %v", nfe.Unwrap())
	}
}

// --- SetupHint: platform-specific output via internal helper ---

func TestSetupHintForGOOS_Darwin(t *testing.T) {
	got := setupHintForGOOS("asana", "darwin")
	if !strings.Contains(got, "devora-asana") {
		t.Errorf("expected service name, got: %q", got)
	}
	if !strings.Contains(got, "security add-generic-password") {
		t.Errorf("expected darwin command, got: %q", got)
	}
}

func TestSetupHintForGOOS_Linux(t *testing.T) {
	got := setupHintForGOOS("asana", "linux")
	if !strings.Contains(got, "devora-asana") {
		t.Errorf("expected service name, got: %q", got)
	}
	if !strings.Contains(got, "secret-tool store") {
		t.Errorf("expected linux command, got: %q", got)
	}
}

func TestSetupHintForGOOS_Windows(t *testing.T) {
	got := setupHintForGOOS("asana", "windows")
	if !strings.Contains(got, "devora-asana") {
		t.Errorf("expected service name, got: %q", got)
	}
	if !strings.Contains(got, "cmdkey") {
		t.Errorf("expected windows command, got: %q", got)
	}
}

func TestSetupHintForGOOS_Fallback(t *testing.T) {
	got := setupHintForGOOS("asana", "plan9")
	if !strings.Contains(got, "devora-asana") {
		t.Errorf("expected service name, got: %q", got)
	}
	if !strings.Contains(got, "keychain") {
		t.Errorf("expected generic keychain hint, got: %q", got)
	}
}

func TestSetupHint_UsesRuntimeGOOS(t *testing.T) {
	// SetupHint should be a thin wrapper that delegates to setupHintForGOOS
	// using runtime.GOOS. Asserting the contained service name keeps the
	// test portable across platforms.
	got := SetupHint("asana")
	if !strings.Contains(got, "devora-asana") {
		t.Errorf("expected service name, got: %q", got)
	}
}
