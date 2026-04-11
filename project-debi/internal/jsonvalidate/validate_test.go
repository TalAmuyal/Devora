package jsonvalidate

import (
	"strings"
	"testing"
)

func TestValidate_ValidInputs(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{"simple object", `{"key": "value"}`},
		{"array", `[1, 2, 3]`},
		{"string", `"hello"`},
		{"number", `42`},
		{"boolean", `true`},
		{"null", `null`},
		{"large nested structure", `{"users": [{"name": "Alice", "age": 30, "active": true}, {"name": "Bob", "age": null, "active": false}], "metadata": {"count": 2, "nested": {"deep": {"value": [1, 2, 3]}}}}`},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if !valid {
				t.Fatalf("expected valid JSON, got errMsg: %s", errMsg)
			}
			if errMsg != "" {
				t.Fatalf("expected empty errMsg, got: %s", errMsg)
			}
		})
	}
}

func TestValidate_InvalidInputs(t *testing.T) {
	tests := []struct {
		name           string
		input          string
		errMsgContains string
	}{
		{"missing closing brace", `{"key": "value"`, "line 1"},
		{"trailing comma", `{"key": "value",}`, ""},
		{"single quotes", `{'key': 'value'}`, ""},
		{"truncated input", `{"key":`, ""},
		{"trailing garbage", `{"a":1}extra`, ""},
		{"whitespace only", "   \n\t  \n  ", ""},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if valid {
				t.Fatal("expected invalid JSON")
			}
			if errMsg == "" {
				t.Fatal("expected non-empty errMsg")
			}
			if tt.errMsgContains != "" && !strings.Contains(errMsg, tt.errMsgContains) {
				t.Fatalf("expected errMsg to contain %q, got: %s", tt.errMsgContains, errMsg)
			}
		})
	}
}

func TestValidate_ErrorPositions(t *testing.T) {
	tests := []struct {
		name              string
		input             string
		errMsgMustContain []string
	}{
		{
			"empty input",
			``,
			[]string{"line 1", "column 1"},
		},
		{
			"multi-line error",
			"{\n  \"key\": \"value\",\n  \"bad\": }\n}",
			[]string{"line 3", "column 10"},
		},
		{
			"first line error",
			`{bad}`,
			[]string{"line 1", "column 2"},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if valid {
				t.Fatal("expected invalid JSON")
			}
			if errMsg == "" {
				t.Fatal("expected non-empty errMsg")
			}
			for _, substr := range tt.errMsgMustContain {
				if !strings.Contains(errMsg, substr) {
					t.Fatalf("expected errMsg to contain %q, got: %s", substr, errMsg)
				}
			}
		})
	}
}

func TestValidate_ErrorMessageFormat(t *testing.T) {
	r := strings.NewReader(`{bad}`)
	valid, errMsg, err := Validate(r)
	if err != nil {
		t.Fatalf("unexpected error: %s", err.Error())
	}
	if valid {
		t.Fatal("expected invalid JSON")
	}
	// Format: "line X, column Y: <error message>"
	if !strings.HasPrefix(errMsg, "line ") {
		t.Fatalf("expected errMsg to start with 'line ', got: %s", errMsg)
	}
	if !strings.Contains(errMsg, ", column ") {
		t.Fatalf("expected errMsg to contain ', column ', got: %s", errMsg)
	}
	if !strings.Contains(errMsg, ": ") {
		t.Fatalf("expected errMsg to contain ': ' separator, got: %s", errMsg)
	}
}
