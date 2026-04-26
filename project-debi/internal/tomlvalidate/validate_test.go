package tomlvalidate

import (
	"strings"
	"testing"
)

func TestValidate_ValidInputs(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{"key=value", `title = "test"`},
		{"integer", `count = 42`},
		{"nested table", "[server]\nhost = \"localhost\"\nport = 8080\n"},
		{"array of tables", "[[fruit]]\nname = \"apple\"\n\n[[fruit]]\nname = \"banana\"\n"},
		{"inline table", `point = { x = 1, y = 2 }`},
		{"comment only", "# just a comment\n"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if !valid {
				t.Fatalf("expected valid TOML, got errMsg: %s", errMsg)
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
		{"empty input", ``, "empty"},
		{"whitespace only", "   \n\t\n", "empty"},
		{"unterminated string", `key = "value`, "line"},
		{"duplicate key", "a = 1\na = 2\n", "line"},
		{"missing equals", "key value\n", "line"},
		{"unmatched bracket", "[section\nkey = 1\n", "line"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if valid {
				t.Fatal("expected invalid TOML")
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
			// BurntSushi/toml reports the column of the redefined key.
			"duplicate key",
			"a = 1\na = 2\n",
			[]string{"line 2, column 7"},
		},
		{
			// Reported at the unterminated string's terminating EOF column.
			"unterminated string",
			`key = "value`,
			[]string{"line 1, column 12"},
		},
		{
			"missing equals",
			"key value\n",
			[]string{"line 1, column 5"},
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
				t.Fatal("expected invalid TOML")
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
	r := strings.NewReader("a = 1\na = 2\n")
	valid, errMsg, err := Validate(r)
	if err != nil {
		t.Fatalf("unexpected error: %s", err.Error())
	}
	if valid {
		t.Fatal("expected invalid TOML")
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
