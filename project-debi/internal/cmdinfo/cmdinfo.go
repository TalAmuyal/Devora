// Package cmdinfo holds the command metadata shared by cli and completion, kept apart from cli so completion can read it without an import cycle.
package cmdinfo

// Command describes a CLI command.
type Command struct {
	Name        string
	Alias       string
	Description string
	ArgsHint    string
	Group       string
	MinArgs     int
	Flags       []Flag
	ValidArgs   []string
	SubCommands []SubCommand
	// CompletesFiles makes the positional argument complete as a file path; flags are offered only once the word starts with "-".
	CompletesFiles bool
}

// SubCommand describes a sub-command accepted by a parent command.
type SubCommand struct {
	Name        string
	Description string
	Flags       []Flag
	ValidArgs   []string
	// CompletesFiles makes the positional argument complete as a file path; flags are offered only once the word starts with "-".
	CompletesFiles bool
}

// Flag describes a flag accepted by a command.
type Flag struct {
	// Name lists every spelling of the flag, comma-separated (e.g. "-v, --verbose").
	Name        string
	Description string
}
