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
}

// SubCommand describes a sub-command accepted by a parent command.
type SubCommand struct {
	Name           string
	Description    string
	Flags          []Flag
	ValidArgs      []string
	CompletesFiles bool
}

// Flag describes a flag accepted by a command.
type Flag struct {
	Name        string
	Description string
}
