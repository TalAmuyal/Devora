package jsonvalidate

import (
	"encoding/json"
	"fmt"
	"io"
)

// Validate reads JSON from the given reader and returns whether it's valid.
// If invalid, errMsg contains a human-readable error with line:column info.
// If there's a read error, it returns it separately.
func Validate(r io.Reader) (valid bool, errMsg string, err error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return false, "", fmt.Errorf("read input: %w", err)
	}

	var v any
	unmarshalErr := json.Unmarshal(data, &v)
	if unmarshalErr != nil {
		syntaxErr, ok := unmarshalErr.(*json.SyntaxError)
		if ok {
			line, col := offsetToLineCol(data, syntaxErr.Offset)
			return false, fmt.Sprintf("line %d, column %d: %s", line, col, syntaxErr.Error()), nil
		}
		// Non-syntax JSON error (e.g., UnmarshalTypeError) — still invalid
		line, col := offsetToLineCol(data, int64(len(data)))
		return false, fmt.Sprintf("line %d, column %d: %s", line, col, unmarshalErr.Error()), nil
	}

	return true, "", nil
}

// offsetToLineCol converts a 1-based byte offset to line and column numbers.
// Offset is 1-based: it represents the number of bytes consumed when the error
// was recorded. An offset of 0 (empty input) maps to line 1, column 1.
func offsetToLineCol(data []byte, offset int64) (line int, col int) {
	if offset <= 0 {
		return 1, 1
	}

	// Clamp offset to data length
	if int(offset) > len(data) {
		offset = int64(len(data))
	}

	prefix := data[:offset]
	line = 1
	lastNewline := -1
	for i, b := range prefix {
		if b == '\n' {
			line++
			lastNewline = i
		}
	}
	col = int(offset) - lastNewline - 1
	return line, col
}
