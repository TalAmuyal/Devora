package cli

import (
	"devora/internal/cmdinfo"
	"devora/internal/git"
	"fmt"
	"strings"
)

// Command describes a CLI command in the registry.
type Command struct {
	Name        string
	Alias       string
	Description string
	ArgsHint    string
	Group       string
	MinArgs     int
	Run         func(args []string) error
	Flags       []cmdinfo.Flag
	ValidArgs   []string
	SubCommands []cmdinfo.SubCommand
}

var groupOrder = []string{
	"Workspace Commands",
	"Health",
	"PR",
	"Git Shortcuts",
	"Utility",
}

var submitFlags = []cmdinfo.Flag{
	{Name: "-m, --message", Description: "Commit message, task title, and PR title (required)"},
	{Name: "-d, --description", Description: "PR body description"},
	{Name: "--draft", Description: "Create draft PR"},
	{Name: "-b, --blocked", Description: "Skip auto-merge for this PR (overrides config)"},
	{Name: "--auto-merge", Description: "Enable auto-merge for this PR (overrides config)"},
	{Name: "-o, --open-browser", Description: "Open PR in browser after creation"},
	{Name: "--skip-tracker", Description: "Skip tracker task creation even if configured"},
	{Name: "--json", Description: "Output result as JSON"},
	{Name: "-v, --verbose", Description: "Show live git/gh subprocess output"},
	{Name: "-q, --quiet", Description: "Print only the final PR URL"},
}

var closeFlags = []cmdinfo.Flag{
	{Name: "-t, --task-url", Description: "Tracker task URL (overrides branch's stored ID)"},
	{Name: "--skip-tracker", Description: "Skip marking tracker task as complete"},
	{Name: "-y, --force", Description: "Skip confirmation prompt for open PRs"},
	{Name: "-v, --verbose", Description: "Show live git/gh subprocess output"},
	{Name: "-q, --quiet", Description: `Print only "Closed" on success`},
}

var autoMergeFlags = []cmdinfo.Flag{
	{Name: "--scope", Description: "Target scope (repo|profile|global); default repo"},
	{Name: "--json", Description: `For "show", emit result as JSON`},
}

var commands = []Command{
	// Workspace Commands
	{
		Name:        "workspace-ui",
		Description: "Open the workspace management UI",
		Group:       "Workspace Commands",
		Run:         func(args []string) error { return runWorkspaceUI() },
	},
	{
		Name:        "add",
		Description: "Add a repo to the current workspace",
		Group:       "Workspace Commands",
		Run:         func(args []string) error { return runAddRepo() },
	},
	{
		Name:        "rename",
		Description: "Rename the current terminal session",
		ArgsHint:    "<new-name>",
		Group:       "Workspace Commands",
		MinArgs:     1,
		Run:         func(args []string) error { return runRename(args[0]) },
	},

	// Health
	{
		Name:        "health",
		Description: "Check Devora dependencies",
		ArgsHint:    "[flags]",
		Group:       "Health",
		Run:         func(args []string) error { return runHealth(args) },
		Flags: []cmdinfo.Flag{
			{Name: "--strict", Description: "Exit with code 1 if any dependency (including optional) is missing"},
			{Name: "-v", Description: "Show dependency locations"},
			{Name: "--verbose", Description: "Show dependency locations"},
			{Name: "-p, --profile", Description: "Check a specific profile (defaults to CWD-based resolution)"},
		},
	},

	// PR commands
	{
		Name:        "pr",
		Description: "Pull request commands",
		ArgsHint:    "<subcommand>",
		Group:       "PR",
		MinArgs:     1,
		Run:         func(args []string) error { return runPR(args) },
		SubCommands: []cmdinfo.SubCommand{
			{
				Name:        "check",
				Description: "Check the status of the PR for the current branch",
				Flags: []cmdinfo.Flag{
					{Name: "--json", Description: "Output status as JSON"},
				},
			},
			{
				Name:        "submit",
				Description: "Commit, create tracker task and GitHub PR",
				Flags:       submitFlags,
			},
			{
				Name:        "close",
				Description: "Complete tracker task, delete branches",
				Flags:       closeFlags,
			},
			{
				Name:        "auto-merge",
				Description: "Manage the per-repo/profile/global pr.auto-merge default",
				Flags:       autoMergeFlags,
				ValidArgs:   []string{"enable", "disable", "reset", "show"},
			},
		},
	},

	// Git Shortcuts — no args
	{
		Name:        "gaa",
		Description: "Stage all changes",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gaa() },
	},
	{
		Name:        "gaac",
		Description: "Stage all and commit with message",
		ArgsHint:    "<msg>",
		Group:       "Git Shortcuts",
		MinArgs:     1,
		Run:         func(args []string) error { return git.Gaac(args) },
	},
	{
		Name:        "gaacp",
		Description: "Stage all, commit, and push to origin",
		ArgsHint:    "<msg>",
		Group:       "Git Shortcuts",
		MinArgs:     1,
		Run:         func(args []string) error { return git.Gaacp(args) },
	},
	{
		Name:        "gaaa",
		Description: "Stage all and amend last commit",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gaaa() },
	},
	{
		Name:        "gaaap",
		Description: "Stage all, amend, and force-push",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gaaap() },
	},
	{
		Name:        "gb",
		Description: "git branch",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gb(args) },
	},
	{
		Name:        "gbd",
		Description: "Force-delete branches",
		ArgsHint:    "<branch>...",
		Group:       "Git Shortcuts",
		MinArgs:     1,
		Run:         func(args []string) error { return git.Gbd(args) },
	},
	{
		Name:        "gbdc",
		Description: "Delete current branch (detach first)",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gbdc() },
	},
	{
		Name:        "gcl",
		Description: "Fetch origin and checkout default branch",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return runGcl(args) },
	},
	{
		Name:        "gcom",
		Description: "Checkout default branch from origin",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gcom(args) },
	},
	{
		Name:        "gd",
		Description: "git diff",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gd(args) },
	},
	{
		Name:        "gfo",
		Description: "Fetch from origin",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gfo(args) },
	},
	{
		Name:        "gg",
		Description: "git grep",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gg(args) },
	},
	{
		Name:        "gl",
		Description: "git log",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gl(args) },
	},
	{
		Name:        "gpo",
		Description: "Push to origin",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gpo(args) },
	},
	{
		Name:        "gpof",
		Description: "Force-push to origin",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gpof(args) },
	},
	{
		Name:        "gpop",
		Description: "Pop git stash",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gpop(args) },
	},
	{
		Name:        "gri",
		Description: "Interactive rebase (N commits or since branch)",
		ArgsHint:    "[N]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gri(args) },
	},
	{
		Name:        "grl",
		Description: "Fetch and rebase on default branch",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Grl() },
	},
	{
		Name:        "grlp",
		Description: "Fetch, rebase, and force-push",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Grlp() },
	},
	{
		Name:        "grom",
		Description: "Rebase on origin default branch",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Grom() },
	},
	{
		Name:        "gst",
		Description: "git status",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return runGst(args) },
	},
	{
		Name:        "gstash",
		Description: "git stash",
		ArgsHint:    "[args]",
		Group:       "Git Shortcuts",
		Run:         func(args []string) error { return git.Gstash(args) },
	},

	// Utility
	{
		Name:        "util",
		Description: "Developer utility commands",
		ArgsHint:    "<subcommand>",
		Group:       "Utility",
		MinArgs:     1,
		Run:         func(args []string) error { return runUtil(args) },
		SubCommands: []cmdinfo.SubCommand{
			{
				Name:           "json-validate",
				Description:    "Validate a JSON file",
				CompletesFiles: true,
			},
			{
				Name:           "yaml-validate",
				Description:    "Validate a YAML file",
				CompletesFiles: true,
			},
			{
				Name:           "toml-validate",
				Description:    "Validate a TOML file",
				CompletesFiles: true,
			},
		},
	},
}

var commandIndex map[string]*Command

func init() {
	// Registered in init to avoid an initialization cycle: the Run closure
	// calls runCompletion, which calls CommandInfos, which reads commands.
	commands = append(commands, Command{
		Name:        "completion",
		Description: "Generate shell completion script",
		ArgsHint:    "<bash|zsh|fish>",
		Group:       "Utility",
		Run:         func(args []string) error { return runCompletion(args) },
		ValidArgs:   []string{"bash", "zsh", "fish"},
	})

	commandIndex = make(map[string]*Command)
	for i := range commands {
		cmd := &commands[i]
		commandIndex[cmd.Name] = cmd
		if cmd.Alias != "" {
			commandIndex[cmd.Alias] = cmd
		}
	}
}

// CommandInfos returns the command metadata from the registry,
// suitable for use by packages that must not import cli (e.g. completion).
func CommandInfos() []cmdinfo.Command {
	result := make([]cmdinfo.Command, len(commands))
	for i, cmd := range commands {
		result[i] = cmdinfo.Command{
			Name:        cmd.Name,
			Alias:       cmd.Alias,
			Description: cmd.Description,
			ArgsHint:    cmd.ArgsHint,
			Group:       cmd.Group,
			MinArgs:     cmd.MinArgs,
			Flags:       cmd.Flags,
			ValidArgs:   cmd.ValidArgs,
			SubCommands: cmd.SubCommands,
		}
	}
	return result
}

// usageMessage generates the usage text from the command registry.
func usageMessage() string {
	// Build the left-column text for each command to determine alignment.
	type entry struct {
		leftCol     string
		description string
	}
	grouped := make(map[string][]entry)
	for _, cmd := range commands {
		left := cmd.Name
		if cmd.Alias != "" {
			left += " (" + cmd.Alias + ")"
		}
		if cmd.ArgsHint != "" {
			left += " " + cmd.ArgsHint
		}
		grouped[cmd.Group] = append(grouped[cmd.Group], entry{left, cmd.Description})
	}

	// Find the global maximum left-column width across all commands.
	maxLeft := 0
	for _, entries := range grouped {
		for _, e := range entries {
			if len(e.leftCol) > maxLeft {
				maxLeft = len(e.leftCol)
			}
		}
	}
	// Pad so the longest entry has a 2-space gap before the description.
	padWidth := maxLeft + 2

	var b strings.Builder
	b.WriteString("usage: debi <command> [args]")

	for _, group := range groupOrder {
		entries := grouped[group]
		if len(entries) == 0 {
			continue
		}
		b.WriteString("\n\n")
		b.WriteString(group + ":")
		for _, e := range entries {
			b.WriteString("\n  ")
			b.WriteString(e.leftCol)
			padding := padWidth - len(e.leftCol)
			b.WriteString(strings.Repeat(" ", padding))
			b.WriteString(e.description)
		}
	}

	return b.String()
}

// usageErrorForCommand returns a UsageError with the standard usage hint
// for a command that requires arguments.
func usageErrorForCommand(cmd *Command) *UsageError {
	hint := cmd.ArgsHint
	if hint == "" {
		hint = "..."
	}
	return &UsageError{
		Message: fmt.Sprintf("usage: debi %s %s", cmd.Name, hint),
	}
}
