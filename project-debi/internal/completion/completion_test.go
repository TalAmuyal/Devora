package completion

import (
	"bytes"
	"devora/internal/cmdinfo"
	"io"
	"strings"
	"testing"
)

// testCommands returns a representative set of commands for testing.
func testCommands() []cmdinfo.Command {
	return []cmdinfo.Command{
		{
			Name:        "workspace-ui",
			Alias:       "w",
			Description: "Open the workspace management UI",
			Group:       "Workspace Commands",
		},
		{
			Name:        "health",
			Description: "Check Devora dependencies",
			ArgsHint:    "[flags]",
			Group:       "Health",
			Flags: []cmdinfo.Flag{
				{Name: "--strict", Description: "Exit with code 1 if any dependency (including optional) is missing"},
				{Name: "-v", Description: "Show dependency locations"},
				{Name: "--verbose", Description: "Show dependency locations"},
			},
		},
		{
			Name:        "pr",
			Description: "Pull request commands",
			ArgsHint:    "<subcommand>",
			Group:       "PR",
			MinArgs:     1,
			ValidArgs:   []string{"status"},
		},
		{
			Name:        "prs",
			Description: "Check the status of the PR for the current branch",
			ArgsHint:    "[flags]",
			Group:       "PR",
			Flags: []cmdinfo.Flag{
				{Name: "--json", Description: "Output status as JSON"},
			},
		},
		{
			Name:        "gaa",
			Description: "Stage all changes",
			Group:       "Git Shortcuts",
		},
		{
			Name:        "completion",
			Description: "Generate shell completion script",
			ArgsHint:    "<bash|zsh|fish>",
			Group:       "Utility",
			ValidArgs:   []string{"bash", "zsh", "fish"},
		},
	}
}

type shellTestCase struct {
	name     string
	generate func(io.Writer, string, []cmdinfo.Command) error
}

func shellGenerators() []shellTestCase {
	return []shellTestCase{
		{name: "bash", generate: GenerateBash},
		{name: "zsh", generate: GenerateZsh},
		{name: "fish", generate: GenerateFish},
	}
}

func TestGenerate_NoError(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			err := tc.generate(&buf, "debi", testCommands())
			if err != nil {
				t.Fatalf("expected no error, got: %s", err)
			}
			if buf.Len() == 0 {
				t.Fatal("expected non-empty output")
			}
		})
	}
}

func TestGenerate_ContainsAllCommandNames(t *testing.T) {
	cmds := testCommands()
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", cmds)
			output := buf.String()

			for _, cmd := range cmds {
				if !strings.Contains(output, cmd.Name) {
					t.Fatalf("%s output should contain command name %q", tc.name, cmd.Name)
				}
			}
		})
	}
}

func TestGenerate_ContainsAliases(t *testing.T) {
	cmds := testCommands()
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", cmds)
			output := buf.String()

			for _, cmd := range cmds {
				if cmd.Alias != "" && !strings.Contains(output, cmd.Alias) {
					t.Fatalf("%s output should contain alias %q for command %q", tc.name, cmd.Alias, cmd.Name)
				}
			}
		})
	}
}

func TestGenerate_ContainsHealthFlags(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			for _, flag := range []string{"--strict", "-v", "--verbose"} {
				if !strings.Contains(output, flag) {
					t.Fatalf("%s output should contain health flag %q", tc.name, flag)
				}
			}
		})
	}
}

func TestGenerate_EmptyCommands_NoError(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			err := tc.generate(&buf, "debi", []cmdinfo.Command{})
			if err != nil {
				t.Fatalf("expected no error for empty commands, got: %s", err)
			}
		})
	}
}

func TestGenerateBash_ContainsBinaryName(t *testing.T) {
	var buf bytes.Buffer
	GenerateBash(&buf, "mybin", testCommands())
	output := buf.String()

	if !strings.Contains(output, "_mybin_completions") {
		t.Fatal("bash output should contain function named _mybin_completions")
	}
	if !strings.Contains(output, "complete -F _mybin_completions mybin") {
		t.Fatal("bash output should register completion for mybin")
	}
}

func TestGenerateZsh_ContainsBinaryName(t *testing.T) {
	var buf bytes.Buffer
	GenerateZsh(&buf, "mybin", testCommands())
	output := buf.String()

	if !strings.Contains(output, "#compdef mybin") {
		t.Fatal("zsh output should start with #compdef mybin")
	}
	if !strings.Contains(output, "_mybin") {
		t.Fatal("zsh output should define _mybin function")
	}
	if !strings.Contains(output, `_mybin "$@"`) {
		t.Fatal("zsh output should end with _mybin invocation")
	}
}

func TestGenerateFish_ContainsBinaryName(t *testing.T) {
	var buf bytes.Buffer
	GenerateFish(&buf, "mybin", testCommands())
	output := buf.String()

	if !strings.Contains(output, "complete -c mybin") {
		t.Fatal("fish output should contain 'complete -c mybin'")
	}
}

func TestGenerateZsh_ContainsDescriptions(t *testing.T) {
	var buf bytes.Buffer
	cmds := testCommands()
	GenerateZsh(&buf, "debi", cmds)
	output := buf.String()

	for _, cmd := range cmds {
		if !strings.Contains(output, cmd.Description) {
			t.Fatalf("zsh output should contain description %q for command %q", cmd.Description, cmd.Name)
		}
	}
}

func TestGenerateFish_ContainsDescriptions(t *testing.T) {
	var buf bytes.Buffer
	cmds := testCommands()
	GenerateFish(&buf, "debi", cmds)
	output := buf.String()

	for _, cmd := range cmds {
		if !strings.Contains(output, cmd.Description) {
			t.Fatalf("fish output should contain description %q for command %q", cmd.Description, cmd.Name)
		}
	}
}

func TestGenerateFish_HealthFlagsOnlyForHealthSubcommand(t *testing.T) {
	var buf bytes.Buffer
	GenerateFish(&buf, "debi", testCommands())
	output := buf.String()

	// Verify that flag completions reference the health subcommand
	if !strings.Contains(output, `__fish_seen_subcommand_from health`) {
		t.Fatal("fish output should use __fish_seen_subcommand_from health for flag completions")
	}
}

func TestGenerateBash_HealthCaseHandlesAlias(t *testing.T) {
	// A command with both flags and an alias should match both name and alias
	cmds := []cmdinfo.Command{
		{
			Name:        "check",
			Alias:       "c",
			Description: "Check things",
			Flags: []cmdinfo.Flag{
				{Name: "--all", Description: "Check all"},
			},
		},
	}
	var buf bytes.Buffer
	GenerateBash(&buf, "debi", cmds)
	output := buf.String()

	if !strings.Contains(output, "check|c)") {
		t.Fatal("bash output should contain 'check|c)' case pattern for aliased command with flags")
	}
}

func TestGenerate_ContainsValidArgs(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			for _, arg := range []string{"bash", "zsh", "fish"} {
				if !strings.Contains(output, arg) {
					t.Fatalf("%s output should contain ValidArgs value %q", tc.name, arg)
				}
			}
		})
	}
}

func TestGenerate_ContainsUniversalHelpFlags(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			for _, flag := range []string{"-h", "--help"} {
				if !strings.Contains(output, flag) {
					t.Fatalf("%s output should contain universal help flag %q", tc.name, flag)
				}
			}
		})
	}
}

func TestGenerateBash_BareCommandNoSubcompletion(t *testing.T) {
	var buf bytes.Buffer
	GenerateBash(&buf, "debi", testCommands())
	output := buf.String()

	// gaa has no Flags and no ValidArgs, so it should NOT appear as a case entry
	if strings.Contains(output, "gaa)") {
		t.Fatal("bash output should not contain a case entry for 'gaa' (bare command with no Flags or ValidArgs)")
	}
}

func TestGenerateFish_ValidArgsForSubcommand(t *testing.T) {
	var buf bytes.Buffer
	GenerateFish(&buf, "debi", testCommands())
	output := buf.String()

	for _, arg := range []string{"bash", "zsh", "fish"} {
		expected := `__fish_seen_subcommand_from completion" -a "` + arg + `"`
		if !strings.Contains(output, expected) {
			t.Fatalf("fish output should contain %q", expected)
		}
	}
}

func TestGenerateZsh_ValidArgsInArgsState(t *testing.T) {
	var buf bytes.Buffer
	GenerateZsh(&buf, "debi", testCommands())
	output := buf.String()

	for _, arg := range []string{"'bash'", "'zsh'", "'fish'"} {
		if !strings.Contains(output, arg) {
			t.Fatalf("zsh output should contain %s in the args state section", arg)
		}
	}
}

func TestGenerate_PRStatusSubcommandCompletion(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			if !strings.Contains(output, "status") {
				t.Fatalf("%s output should contain 'status' as a completion option for pr", tc.name)
			}
		})
	}
}

func TestGenerate_PRSJsonFlagCompletion(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			if !strings.Contains(output, "--json") {
				t.Fatalf("%s output should contain '--json' as a completion option for prs", tc.name)
			}
		})
	}
}
