package judgess

import (
	"strings"
	"testing"
)

func TestParseInput_ExtractsFields(t *testing.T) {
	const payload = `{"tool_name":"Bash","tool_input":{"command":"ls -la"},"cwd":"/tmp/work"}`

	in, err := parseInput(strings.NewReader(payload))
	if err != nil {
		t.Fatalf("unexpected error: %s", err.Error())
	}
	if in.ToolName != "Bash" {
		t.Errorf("tool_name: got %q, want %q", in.ToolName, "Bash")
	}
	if in.Cwd != "/tmp/work" {
		t.Errorf("cwd: got %q, want %q", in.Cwd, "/tmp/work")
	}
	if got := string(in.ToolInput); got != `{"command":"ls -la"}` {
		t.Errorf("tool_input: got %q, want %q", got, `{"command":"ls -la"}`)
	}
}

func TestRun_AbstainsOnAllTools(t *testing.T) {
	cases := []struct {
		name    string
		payload string
	}{
		{"bash", `{"tool_name":"Bash","tool_input":{"command":"ls"},"cwd":"/tmp"}`},
		{"read", `{"tool_name":"Read","tool_input":{"file_path":"/tmp/a"},"cwd":"/tmp"}`},
		{"webfetch", `{"tool_name":"WebFetch","tool_input":{"url":"https://example.com"},"cwd":"/tmp"}`},
		{"ask_user_question", `{"tool_name":"AskUserQuestion","tool_input":{},"cwd":"/tmp"}`},
		{"edit", `{"tool_name":"Edit","tool_input":{"file_path":"/tmp/a"},"cwd":"/tmp"}`},
		{"write", `{"tool_name":"Write","tool_input":{"file_path":"/tmp/a"},"cwd":"/tmp"}`},
		{"unknown_tool", `{"tool_name":"SomeFutureTool","tool_input":{"x":1},"cwd":"/tmp"}`},
		{"missing_optional_fields", `{"tool_name":"Bash"}`},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if err := Run(strings.NewReader(tc.payload)); err != nil {
				t.Fatalf("expected abstain (nil), got error: %s", err.Error())
			}
		})
	}
}

func TestRun_MalformedInput_ReturnsError(t *testing.T) {
	cases := []struct {
		name    string
		payload string
	}{
		{"not_json", `not json at all`},
		{"empty", ``},
		{"truncated", `{"tool_name":`},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if err := Run(strings.NewReader(tc.payload)); err == nil {
				t.Fatal("expected an error for malformed input, got nil")
			}
		})
	}
}
