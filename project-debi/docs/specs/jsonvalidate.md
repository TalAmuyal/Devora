# JSON Validate Package Spec

Package: `internal/jsonvalidate`

## Purpose

Validate JSON input and report errors with human-readable line and column positions.

## Function

### Validate

```go
func Validate(r io.Reader) (valid bool, errMsg string, err error)
```

Reads all bytes from `r`, then validates the content as JSON.

**Parameters:**
- `r` — the source of JSON data to validate.

**Returns:**
- `valid` — true if the input is valid JSON, false otherwise.
- `errMsg` — when invalid, a human-readable message in the format `"line X, column Y: <error detail>"`. Empty when valid.
- `err` — non-nil only for I/O errors (not JSON syntax errors).

**Behavior:**
1. Read all bytes from `r` using `io.ReadAll`.
2. Attempt `json.Unmarshal` into `any`. This catches trailing garbage that `json.Decoder` would miss.
3. On success, return `valid=true`.
4. On `*json.SyntaxError`, convert the 1-based byte offset to line and column, then format the error message.
5. On other JSON errors, report position at end of input.

### offsetToLineCol (unexported)

```go
func offsetToLineCol(data []byte, offset int64) (line int, col int)
```

Converts a 1-based byte offset to line and column numbers.

- Count newlines in `data[:offset]` to determine the line.
- Column is the distance from the last newline to the offset position.
- Edge case: offset <= 0 (e.g., empty input) maps to line 1, column 1.
