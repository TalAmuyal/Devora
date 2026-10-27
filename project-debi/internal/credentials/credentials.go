// Package credentials fetches API tokens from the OS keychain.
//
// Entries live under service "devora-<provider>" with the user's login as the account.
// go-keyring is deliberately the sole backend; tokens are never read from files or other fallback stores.
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

// NotFoundError's Error() stays single-line; callers print the multi-line SetupHint(Provider) under it.
type NotFoundError struct {
	Provider string
	Service  string
	Account  string
}

func (e *NotFoundError) Error() string {
	return fmt.Sprintf("no '%s' credential stored in keychain for account '%s'", e.Service, e.Account)
}

// Stubbable package-level vars for tests.
var (
	keyringGet = keyring.Get
	getUser    = defaultGetUser
	timeout    = 5 * time.Second
)

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
// The timeout guards against a macOS keychain prompt that blocks indefinitely when dismissed or unfocused.
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

func SetupHint(provider string) string {
	return setupHintForGOOS(provider, runtime.GOOS)
}

// setupHintForGOOS takes goos explicitly so tests can cover every platform.
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
