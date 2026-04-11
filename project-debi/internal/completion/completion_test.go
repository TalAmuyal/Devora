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
			SubCommands: []cmdinfo.SubCommand{
				{
					Name:        "status",
					Description: "Check the status of the PR for the current branch",
					Flags: []cmdinfo.Flag{
						{Name: "--json", Description: "Output status as JSON"},
					},
				},
			},
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
		{
			Name:        "util",
			Description: "Developer utility commands",
			ArgsHint:    "<subcommand>",
			Group:       "Utility",
			MinArgs:     1,
			SubCommands: []cmdinfo.SubCommand{
				{
					Name:           "json-validate",
					Description:    "Validate a JSON file",
					CompletesFiles: true,
				},
			},
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

func TestGenerate_SubCommandNamesAtSecondLevel(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			// Sub-command names should appear in second-level completions
			if !strings.Contains(output, "json-validate") {
				t.Fatalf("%s output should contain 'json-validate' as a sub-command of util", tc.name)
			}
			if !strings.Contains(output, "status") {
				t.Fatalf("%s output should contain 'status' as a sub-command of pr", tc.name)
			}
		})
	}
}

func TestGenerateBash_SubCommandFlags(t *testing.T) {
	var buf bytes.Buffer
	GenerateBash(&buf, "debi", testCommands())
	output := buf.String()

	// Third-level: pr:status should offer --json
	if !strings.Contains(output, "pr:status)") {
		t.Fatal("bash output should contain 'pr:status)' case for third-level completion")
	}
	if !strings.Contains(output, `compgen -W "--json`) {
		t.Fatal("bash output should contain compgen -W with --json for pr:status")
	}
}

func TestGenerateZsh_SubCommandFlags(t *testing.T) {
	var buf bytes.Buffer
	GenerateZsh(&buf, "debi", testCommands())
	output := buf.String()

	// pr's args section should dispatch on status and offer --json
	if !strings.Contains(output, "--json") {
		t.Fatal("zsh output should contain '--json' in pr status context")
	}
}

func TestGenerateFish_SubCommandFlags(t *testing.T) {
	var buf bytes.Buffer
	GenerateFish(&buf, "debi", testCommands())
	output := buf.String()

	// Fish should use seen_subcmd for third-level completions
	if !strings.Contains(output, "__debi_seen_subcmd") {
		t.Fatal("fish output should contain '__debi_seen_subcmd' helper function")
	}
	if !strings.Contains(output, "--json") {
		t.Fatal("fish output should contain '--json' for pr status")
	}
}

func TestGenerateBash_SubCommandFileCompletion(t *testing.T) {
	var buf bytes.Buffer
	GenerateBash(&buf, "debi", testCommands())
	output := buf.String()

	if !strings.Contains(output, "util:json-validate)") {
		t.Fatal("bash output should contain 'util:json-validate)' case for third-level file completion")
	}
	if !strings.Contains(output, "compgen -f") {
		t.Fatal("bash output should contain 'compgen -f' for file completion in util:json-validate")
	}
}

func TestGenerateZsh_SubCommandFileCompletion(t *testing.T) {
	var buf bytes.Buffer
	GenerateZsh(&buf, "debi", testCommands())
	output := buf.String()

	if !strings.Contains(output, "json-validate") {
		t.Fatal("zsh output should contain 'json-validate' in util sub-command handling")
	}
	if !strings.Contains(output, "_files") {
		t.Fatal("zsh output should contain '_files' for file completion in util json-validate")
	}
}

func TestGenerateFish_SubCommandFileCompletion(t *testing.T) {
	var buf bytes.Buffer
	GenerateFish(&buf, "debi", testCommands())
	output := buf.String()

	if !strings.Contains(output, "__debi_seen_subcmd util json-validate") {
		t.Fatal("fish output should contain '__debi_seen_subcmd util json-validate' for file completion")
	}
	// File completion uses -F flag in fish
	if !strings.Contains(output, "-F") {
		t.Fatal("fish output should contain '-F' flag for file completion in util json-validate")
	}
}

func TestGenerateFish_HelperFunctionsExist(t *testing.T) {
	var buf bytes.Buffer
	GenerateFish(&buf, "debi", testCommands())
	output := buf.String()

	if !strings.Contains(output, "__debi_needs_subcmd_of") {
		t.Fatal("fish output should contain '__debi_needs_subcmd_of' helper function")
	}
	if !strings.Contains(output, "__debi_seen_subcmd") {
		t.Fatal("fish output should contain '__debi_seen_subcmd' helper function")
	}
}

func TestGenerate_CommandsWithoutSubCommandsUnaffected(t *testing.T) {
	for _, tc := range shellGenerators() {
		t.Run(tc.name, func(t *testing.T) {
			var buf bytes.Buffer
			tc.generate(&buf, "debi", testCommands())
			output := buf.String()

			// health, completion, gaa should still appear
			for _, name := range []string{"health", "completion", "gaa"} {
				if !strings.Contains(output, name) {
					t.Fatalf("%s output should still contain command %q", tc.name, name)
				}
			}
			// health flags should still work
			for _, flag := range []string{"--strict", "-v", "--verbose"} {
				if !strings.Contains(output, flag) {
					t.Fatalf("%s output should still contain health flag %q", tc.name, flag)
				}
			}
		})
	}
}

func TestGenerateFish_NoHelperFunctionsWithoutSubCommands(t *testing.T) {
	// When no commands have SubCommands, helper functions should not appear
	cmds := []cmdinfo.Command{
		{
			Name:        "health",
			Description: "Check Devora dependencies",
			Flags: []cmdinfo.Flag{
				{Name: "--strict", Description: "Strict mode"},
			},
		},
	}
	var buf bytes.Buffer
	GenerateFish(&buf, "debi", cmds)
	output := buf.String()

	if strings.Contains(output, "__debi_needs_subcmd_of") {
		t.Fatal("fish output should not contain '__debi_needs_subcmd_of' when no commands have SubCommands")
	}
	if strings.Contains(output, "__debi_seen_subcmd") {
		t.Fatal("fish output should not contain '__debi_seen_subcmd' when no commands have SubCommands")
	}
}

func TestGenerateBash_SubCommandSecondLevelFromSubCommands(t *testing.T) {
	// For commands with SubCommands, second-level should show sub-command names
	var buf bytes.Buffer
	GenerateBash(&buf, "debi", testCommands())
	output := buf.String()

	// pr should offer "status" at second level
	if !strings.Contains(output, "pr)") {
		t.Fatal("bash output should contain 'pr)' case entry for second-level completion")
	}
	// util should offer "json-validate" at second level
	if !strings.Contains(output, "util)") {
		t.Fatal("bash output should contain 'util)' case entry for second-level completion")
	}
}
