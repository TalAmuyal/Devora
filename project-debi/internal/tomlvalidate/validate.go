package tomlvalidate

import (
	"bytes"
	"errors"
	"fmt"
	"io"

	"github.com/BurntSushi/toml"
)

// Validate reads TOML from the given reader and returns whether it's valid.
// If invalid, errMsg contains a human-readable error with line:column info.
// If there's a read error, it returns it separately.
func Validate(r io.Reader) (valid bool, errMsg string, err error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return false, "", fmt.Errorf("read input: %w", err)
	}

	if len(bytes.TrimSpace(data)) == 0 {
		return false, "line 1, column 1: empty input", nil
	}

	var v any
	_, decodeErr := toml.Decode(string(data), &v)
	if decodeErr == nil {
		return true, "", nil
	}

	var pe toml.ParseError
	if errors.As(decodeErr, &pe) {
		return false, fmt.Sprintf("line %d, column %d: %s", pe.Position.Line, pe.Position.Col, pe.Message), nil
	}

	return false, fmt.Sprintf("line 1, column 1: %s", decodeErr.Error()), nil
}
