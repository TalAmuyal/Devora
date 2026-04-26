# TOML Validate Package Spec

Package: `internal/tomlvalidate`

## Purpose

Validate TOML input and report errors with human-readable line and column positions.

## Function

### Validate

```go
func Validate(r io.Reader) (valid bool, errMsg string, err error)
```

Reads all bytes from `r`, then validates the content as TOML.

**Parameters:**
- `r` — the source of TOML data to validate.

**Returns:**
- `valid` — true if the input is valid TOML, false otherwise.
- `errMsg` — when invalid, a human-readable message in the format `"line X, column Y: <error detail>"`. Empty when valid.
- `err` — non-nil only for I/O errors (not TOML syntax errors).

**Behavior:**
1. Read all bytes from `r` using `io.ReadAll`.
2. If the input is empty after trimming whitespace (`bytes.TrimSpace(data)` is empty), return `valid=false` with `errMsg = "line 1, column 1: empty input"`. A comment-only file is *not* whitespace-only and proceeds to step 3, where it parses successfully.
3. Decode via `toml.Decode(string(data), &v)` with `var v any`.
4. On success, return `valid=true`.
5. On error: if `errors.As(err, &pe)` succeeds for `pe toml.ParseError`, format using `pe.Position.Line`, `pe.Position.Col`, and `pe.Message`. Otherwise, fall back to `"line 1, column 1: " + err.Error()`.

## Dependencies

- `github.com/BurntSushi/toml` (MIT). Provides `toml.Decode` and the typed `toml.ParseError` (value type) with a `Position{Line, Col, Start, Len}` field and a clean `Message` string.
