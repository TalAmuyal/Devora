package yamlvalidate

import (
	"bytes"
	"errors"
	"fmt"
	"io"

	"github.com/goccy/go-yaml"
)

// Validate reads YAML from the given reader and returns whether it's valid.
// All documents in a multi-document stream are validated; any failure marks
// the input as invalid. If invalid, errMsg contains a human-readable error
// with line:column info. If there's a read error, it returns it separately.
func Validate(r io.Reader) (valid bool, errMsg string, err error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return false, "", fmt.Errorf("read input: %w", err)
	}

	if len(bytes.TrimSpace(data)) == 0 {
		return false, "line 1, column 1: empty input", nil
	}

	dec := yaml.NewDecoder(bytes.NewReader(data))
	for {
		var v any
		decodeErr := dec.Decode(&v)
		if errors.Is(decodeErr, io.EOF) {
			return true, "", nil
		}
		if decodeErr != nil {
			return false, formatYAMLError(decodeErr), nil
		}
	}
}

// formatYAMLError converts a goccy/go-yaml error into the canonical
// "line X, column Y: <detail>" format. goccy's typed errors (SyntaxError,
// TypeError, etc.) all implement yaml.Error, exposing the offending token's
// position and a clean message. Errors without a token (or without a typed
// position) fall back to line 1, column 1.
func formatYAMLError(err error) string {
	var typedErr yaml.Error
	if errors.As(err, &typedErr) {
		tok := typedErr.GetToken()
		msg := typedErr.GetMessage()
		if tok != nil {
			return fmt.Sprintf("line %d, column %d: %s", tok.Position.Line, tok.Position.Column, msg)
		}
		return fmt.Sprintf("line 1, column 1: %s", msg)
	}
	return fmt.Sprintf("line 1, column 1: %s", err.Error())
}
