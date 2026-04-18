# Credentials Package Spec

Package: `internal/credentials`

## Purpose

Fetch API tokens from the OS keychain, via a single backend with no fallbacks. Used by `internal/tasktracker/asana` (to look up the Asana token) and by `internal/health` (to surface a credential row when a task-tracker is configured).

## Stability

Stable public API: `GetToken`, `SetupHint`, `ErrNotFound`, `NotFoundError`.

## Dependencies

- `github.com/zalando/go-keyring` -- the sole keychain backend

## Types

### NotFoundError

```go
type NotFoundError struct {
    Provider string
    Service  string // "devora-<provider>"
    Account  string // the resolved user login
}
```

Returned when the keychain has no entry for the provider or the stored value is empty. `Error()` returns a short single-line message. `Unwrap()` returns `ErrNotFound` so `errors.Is(err, ErrNotFound)` works on a wrapped `*NotFoundError`.

Callers typically use `errors.As` to extract `Provider` and then call `SetupHint(Provider)` to print multi-line setup instructions alongside the short error.

## Sentinels

```go
var ErrNotFound = errors.New("credential not found")
```

Returned (via `*NotFoundError.Unwrap`) when no credential is stored. Use with `errors.Is` on wrapped errors.

## Stubbable Dependencies

```go
var (
    keyringGet = keyring.Get
    getUser    = defaultGetUser
    timeout    = 5 * time.Second
)
```

Tests replace `keyringGet` (to simulate keychain responses via `keyring.MockInit`), `getUser` (to exercise user-resolution branches), and `timeout` (to exercise the timeout path without waiting 5 seconds).

## User Resolution

`defaultGetUser` prefers `$USER` and falls through to `os/user.Current().Username` when the env var is empty. An empty result from both paths produces an error; `GetToken` returns that error unchanged.

## Functions

### GetToken

```go
func GetToken(provider string) (string, error)
```

Retrieves the API token for `provider` from the OS keychain.

- Service name: `"devora-" + provider`.
- Account: the resolved user login (see above).

The keychain call is wrapped in a 5-second timeout because macOS's `security` can surface an OS-level prompt that blocks indefinitely when dismissed or not focused. The implementation runs `keyringGet(service, user)` in a goroutine and selects on `time.After(timeout)`.

Return values:

- Success: `(token, nil)` with a non-empty token.
- Missing entry or empty stored value: `("", *NotFoundError)`.
- Timeout: an error whose message contains `"timed out"`.
- Unsupported platform (`keyring.ErrUnsupportedPlatform`): wrapped error explaining that debi credentials require macOS/Linux/Windows keychains.
- Other keychain errors: wrapped with a `"keychain:"` prefix.

### SetupHint

```go
func SetupHint(provider string) string
```

Returns platform-specific multi-line instructions for storing a provider's token. Callers that receive `*NotFoundError` typically print this under the short error message.

| GOOS | Command |
|---|---|
| `darwin` | `security add-generic-password -s devora-<provider> -a "$USER" -w` |
| `linux` | `secret-tool store --label="devora-<provider>" service devora-<provider> account "$USER"` |
| `windows` | `cmdkey /generic:devora-<provider> /user:%USERNAME% /pass` |
| anything else | Generic "store the token in your OS keychain under service `"devora-<provider>"` and account equal to `$USER`" message |

`SetupHint` dispatches on `runtime.GOOS` via an internal `setupHintForGOOS(provider, goos string)` helper so tests can cover every branch without overriding the runtime global.

## Keychain Backend

The backend is `github.com/zalando/go-keyring`. On macOS this wraps the `security` command; on Linux it uses Secret Service via libsecret; on Windows it uses the Credential Manager. No fallback to files or other stores is implemented.

## Test Coverage

`internal/credentials/credentials_test.go` covers:

- Happy path returns the stored token.
- Missing entry returns `*NotFoundError`.
- Empty stored value returns `*NotFoundError`.
- `keyring.ErrUnsupportedPlatform` is wrapped.
- Generic keychain errors are wrapped with a `"keychain:"` prefix.
- Timeout returns an error mentioning `"timed out"`.
- `$USER` unset falls through to `os/user.Current`.
- Full user-resolution failure propagates.
- `defaultGetUser` prefers `$USER` when set.
- `NotFoundError.Error()` is single-line; `Unwrap()` returns `ErrNotFound`.
- `setupHintForGOOS` branches for darwin, linux, windows, and the fallback.
- `SetupHint` delegates to `runtime.GOOS`.
