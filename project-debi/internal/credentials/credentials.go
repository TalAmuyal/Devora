// Package credentials fetches API tokens from the OS keychain.
//
// The service name convention is "devora-<provider>" and the account is the
// current user's login. The backend is github.com/zalando/go-keyring — a
// single backend with no fallbacks. Callers that receive *NotFoundError
// should print SetupHint(provider) to guide the user on how to store a token.
package credentials

import (
	"errors"
	"fmt"
	"os"
	ossuser "os/user"
	"runtime"
	"time"

	"github.com/zalando/go-keyring"
)

// ErrNotFound is the sentinel returned (via *NotFoundError.Unwrap) when no
// credential is stored. Use with errors.Is on a wrapped *NotFoundError.
var ErrNotFound = errors.New("credential not found")

// NotFoundError is returned when the keychain has no entry for the provider.
// Callers typically use errors.As to extract the Provider and then print
// SetupHint(Provider) alongside the short message from Error().
type NotFoundError struct {
	Provider string
	Service  string // "devora-<provider>"
	Account  string // resolved user login
}

// Error returns a short single-line message. Callers needing the multi-line
// setup instructions should call SetupHint(provider).
func (e *NotFoundError) Error() string {
	return fmt.Sprintf("no '%s' credential stored in keychain for account '%s'", e.Service, e.Account)
}

// Unwrap makes errors.Is(err, ErrNotFound) work on any wrapped NotFoundError.
func (e *NotFoundError) Unwrap() error { return ErrNotFound }

// Stubbable package-level vars for tests.
var (
	keyringGet = keyring.Get
	getUser    = defaultGetUser
	timeout    = 5 * time.Second
)

// defaultGetUser resolves the current user's login. It prefers $USER and
// falls through to os/user.Current. It returns an error only if both paths
// fail to yield a non-empty username.
func defaultGetUser() (string, error) {
	if u := os.Getenv("USER"); u != "" {
		return u, nil
	}
	cu, err := ossuser.Current()
	if err != nil {
		return "", fmt.Errorf("cannot determine current user: $USER not set and os/user.Current failed: %w", err)
	}
	if cu.Username == "" {
		return "", errors.New("cannot determine current user: $USER not set and os/user.Current returned empty username")
	}
	return cu.Username, nil
}

// GetToken retrieves the API token for a provider from the OS keychain.
//
// The service name is "devora-<provider>"; the account is the resolved user
// login. The keychain call is wrapped in a timeout (default 5s) because
// macOS can surface an OS-level prompt that blocks indefinitely if the user
// dismisses it or the prompt is not focused.
//
// Returns a *NotFoundError when the entry is missing (or the stored value is
// empty). Returns a timeout error whose message contains "timed out" when
// the keychain call does not complete within the configured timeout.
// Wraps keyring.ErrUnsupportedPlatform with a clear message on unsupported
// platforms. Other keychain errors are wrapped with a "keychain:" prefix.
func GetToken(provider string) (string, error) {
	user, err := getUser()
	if err != nil {
		return "", err
	}
	service := "devora-" + provider

	type result struct {
		token string
		err   error
	}
	ch := make(chan result, 1)
	go func() {
		tok, gerr := keyringGet(service, user)
		ch <- result{tok, gerr}
	}()

	select {
	case <-time.After(timeout):
		return "", fmt.Errorf("keychain access timed out after %s (was the OS prompt dismissed or not focused?)", timeout)
	case r := <-ch:
		if r.err != nil {
			if errors.Is(r.err, keyring.ErrNotFound) {
				return "", &NotFoundError{Provider: provider, Service: service, Account: user}
			}
			if errors.Is(r.err, keyring.ErrUnsupportedPlatform) {
				return "", fmt.Errorf("keychain unsupported on this platform; debi credentials require macOS/Linux/Windows keychains: %w", r.err)
			}
			return "", fmt.Errorf("keychain: %w", r.err)
		}
		if r.token == "" {
			return "", &NotFoundError{Provider: provider, Service: service, Account: user}
		}
		return r.token, nil
	}
}

// SetupHint returns platform-specific multi-line instructions for storing a
// provider's token in the OS keychain. Callers that receive *NotFoundError
// typically print this under the short error message.
func SetupHint(provider string) string {
	return setupHintForGOOS(provider, runtime.GOOS)
}

// setupHintForGOOS is the testable core of SetupHint. It accepts an explicit
// goos string so tests can exercise every branch without overriding the
// runtime global.
func setupHintForGOOS(provider, goos string) string {
	service := "devora-" + provider
	switch goos {
	case "darwin":
		return fmt.Sprintf(`Store the %s token with:

  security add-generic-password -s %s -a "$USER" -w

(the command will prompt for the token)`, provider, service)
	case "linux":
		return fmt.Sprintf(`Store the %s token with:

  secret-tool store --label="%s" service %s account "$USER"

(the command will prompt for the token)`, provider, service, service)
	case "windows":
		return fmt.Sprintf(`Store the %s token with:

  cmdkey /generic:%s /user:%%USERNAME%% /pass

(the command will prompt for the token)`, provider, service)
	default:
		return fmt.Sprintf("Store the %s token in your OS keychain under service %q and account equal to $USER.", provider, service)
	}
}
