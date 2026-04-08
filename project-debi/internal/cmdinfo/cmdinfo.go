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
}

// Flag describes a flag accepted by a command.
type Flag struct {
	Name        string
	Description string
}
