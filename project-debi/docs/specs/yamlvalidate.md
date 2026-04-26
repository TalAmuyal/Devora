# YAML Validate Package Spec

Package: `internal/yamlvalidate`

## Purpose

Validate YAML input and report errors with human-readable line and column positions. Multi-document YAML streams are fully supported: every document is parsed and any failure marks the input as invalid.

## Function

### Validate

```go
func Validate(r io.Reader) (valid bool, errMsg string, err error)
```

Reads all bytes from `r`, then validates the content as YAML.

**Parameters:**
- `r` — the source of YAML data to validate.

**Returns:**
- `valid` — true if the input is valid YAML, false otherwise.
- `errMsg` — when invalid, a human-readable message in the format `"line X, column Y: <error detail>"`. Empty when valid.
- `err` — non-nil only for I/O errors (not YAML syntax errors).

**Behavior:**
1. Read all bytes from `r` using `io.ReadAll`.
2. If the input is empty after trimming whitespace (`bytes.TrimSpace(data)` is empty), return `valid=false` with `errMsg = "line 1, column 1: empty input"`.
3. Stream the bytes through `yaml.NewDecoder(...).Decode(&v)` (with `var v any`) until `io.EOF`. This validates every document in a multi-document YAML stream.
4. On the first non-EOF error, format it via `formatYAMLError` and return `valid=false`.
5. On `io.EOF`, return `valid=true`.

### formatYAMLError (unexported)

```go
func formatYAMLError(err error) string
```

Converts a `goccy/go-yaml` error into the canonical `"line X, column Y: <detail>"` format.

- The library's typed errors (`SyntaxError`, `TypeError`, `OverflowError`, `DuplicateKeyError`, `UnknownFieldError`, `UnexpectedNodeTypeError`) all satisfy the `yaml.Error` interface, which exposes the offending token via `GetToken()` and a clean message via `GetMessage()`. Position is read from `tok.Position.Line` and `tok.Position.Column`.
- If `errors.As(err, &yaml.Error)` matches but the token is nil, fall back to `line 1, column 1`.
- If the typed-error path does not match, fall back to `"line 1, column 1: " + err.Error()`.

## Dependencies

- `github.com/goccy/go-yaml` (MIT). Provides parser, decoder, and typed errors with `Token.Position` line/column information.
