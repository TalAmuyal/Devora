package yamlvalidate

import (
	"strings"
	"testing"
)

func TestValidate_ValidInputs(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{"scalar", `hello`},
		{"flow mapping", `{key: value}`},
		{"flow sequence", `[1, 2, 3]`},
		{"block mapping", "key: value\nother: 7\n"},
		{"nested", "users:\n  - name: Alice\n    age: 30\n  - name: Bob\n    age: 25\n"},
		{"multi-document", "---\nfoo: 1\n---\nbar: 2\n"},
		{"trailing document marker", "foo: 1\n---\n"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if !valid {
				t.Fatalf("expected valid YAML, got errMsg: %s", errMsg)
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
		{"unclosed flow", `{key: value`, "line 1"},
		{"tab indentation", "key:\n\tvalue: 1", "line"},
		{"second doc invalid", "foo: 1\n---\n{bad: \n", "line"},
		{"unclosed flow sequence", `[1, 2,`, "line"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := strings.NewReader(tt.input)
			valid, errMsg, err := Validate(r)
			if err != nil {
				t.Fatalf("unexpected error: %s", err.Error())
			}
			if valid {
				t.Fatal("expected invalid YAML")
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
			// goccy/go-yaml reports the start of the unclosed flow scalar.
			"unclosed flow",
			`{key: value`,
			[]string{"line 1, column 1"},
		},
		{
			// Tab indentation lands on the second line where the tab appears.
			"tab indentation",
			"key:\n\tvalue: 1",
			[]string{"line 2, column 1"},
		},
		{
			// Error inside a second YAML document; goccy reports the position
			// inside that document, not the start of the input.
			"second document invalid",
			"foo: 1\n---\n{bad: \n",
			[]string{"line 3, column 5"},
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
				t.Fatal("expected invalid YAML")
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
	r := strings.NewReader(`{key: value`)
	valid, errMsg, err := Validate(r)
	if err != nil {
		t.Fatalf("unexpected error: %s", err.Error())
	}
	if valid {
		t.Fatal("expected invalid YAML")
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
