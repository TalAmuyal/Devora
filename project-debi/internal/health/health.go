package health

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"time"

	"charm.land/lipgloss/v2"

	"devora/internal/process"
)

var homeDir = os.Getenv("HOME")

type Dependency struct {
	Name           string
	Required       bool
	VersionCommand []string
}

type CheckResult struct {
	Dependency
	Found   bool
	Path    string
	Version string
}

var lookPath = exec.LookPath

var getVersion = defaultGetVersion

func defaultGetVersion(command []string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, command[0], command[1:]...)
	var stdout bytes.Buffer
	cmd.Stdout = &stdout
	err := cmd.Run()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(stdout.String()), nil
}

var dependencies = []Dependency{
	{Name: "kitty", Required: true, VersionCommand: []string{"kitty", "--version"}},
	{Name: "claude", Required: true, VersionCommand: []string{"claude", "--version"}},
	{Name: "git", Required: true, VersionCommand: []string{"git", "--version"}},
	{Name: "uv", Required: true, VersionCommand: []string{"uv", "--version"}},
	{Name: "glow", Required: true, VersionCommand: []string{"glow", "--version"}},
	{Name: "zsh", Required: true, VersionCommand: []string{"zsh", "--version"}},
	{Name: "nvim", Required: false, VersionCommand: []string{"nvim", "--version"}},
	{Name: "mise", Required: false, VersionCommand: []string{"mise", "--version"}},
	{Name: "jq", Required: false, VersionCommand: []string{"jq", "--version"}},
}

var versionPattern = regexp.MustCompile(`v?\d+(?:\.\d+)+`)

func cleanVersion(raw string) string {
	match := versionPattern.FindString(raw)
	if match != "" {
		return strings.TrimPrefix(match, "v")
	}
	return raw
}

func shortenPath(path string) string {
	if homeDir != "" && strings.HasPrefix(path, homeDir) {
		return "~" + path[len(homeDir):]
	}
	return path
}

func Check(dep Dependency) CheckResult {
	result := CheckResult{Dependency: dep}

	path, err := lookPath(dep.Name)
	if err != nil {
		return result
	}
	result.Found = true
	result.Path = path

	output, err := getVersion(dep.VersionCommand)
	if err != nil {
		return result
	}

	firstLine, _, _ := strings.Cut(output, "\n")
	result.Version = cleanVersion(strings.TrimSpace(firstLine))

	return result
}

func renderSection(w io.Writer, results []CheckResult, nameWidth, versionWidth int, verbose bool, greenStyle, redStyle lipgloss.Style) int {
	found := 0
	for _, r := range results {
		if r.Found {
			found++
			prefix := greenStyle.Render(fmt.Sprintf("  \u2713 %-*s", nameWidth, r.Name))
			if verbose {
				fmt.Fprintf(w, "%s  %-*s  %s\n", prefix, versionWidth, r.Version, shortenPath(r.Path))
			} else {
				fmt.Fprintf(w, "%s  %s\n", prefix, r.Version)
			}
		} else {
			prefix := redStyle.Render(fmt.Sprintf("  \u2717 %-*s", nameWidth, r.Name))
			if verbose {
				fmt.Fprintf(w, "%s  %-*s  not found\n", prefix, versionWidth, "")
			} else {
				fmt.Fprintf(w, "%s  not found\n", prefix)
			}
		}
	}
	return found
}

func Run(w io.Writer, strict bool, verbose bool) error {
	var required []CheckResult
	var optional []CheckResult

	for _, dep := range dependencies {
		result := Check(dep)
		if dep.Required {
			required = append(required, result)
		} else {
			optional = append(optional, result)
		}
	}

	nameWidth := 0
	versionWidth := 0
	for _, r := range required {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
		if len(r.Version) > versionWidth {
			versionWidth = len(r.Version)
		}
	}
	for _, r := range optional {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
		if len(r.Version) > versionWidth {
			versionWidth = len(r.Version)
		}
	}

	greenStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#A6E3A1"))
	redStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#F38BA8"))

	fmt.Fprintln(w, "Devora Health Check")
	fmt.Fprintln(w)

	fmt.Fprintln(w, "Required:")
	requiredFound := renderSection(w, required, nameWidth, versionWidth, verbose, greenStyle, redStyle)

	fmt.Fprintln(w)

	fmt.Fprintln(w, "Optional:")
	optionalFound := renderSection(w, optional, nameWidth, versionWidth, verbose, greenStyle, redStyle)

	fmt.Fprintln(w)

	yellowStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#F9E2AF"))

	requiredPct := 0
	if len(required) > 0 {
		requiredPct = requiredFound * 100 / len(required)
	}
	optionalPct := 0
	if len(optional) > 0 {
		optionalPct = optionalFound * 100 / len(optional)
	}

	requiredPctStyle := greenStyle
	if requiredFound < len(required) {
		requiredPctStyle = redStyle
	}
	optionalPctStyle := greenStyle
	if optionalFound < len(optional) {
		optionalPctStyle = yellowStyle
	}

	fmt.Fprintf(w, "Required met: %s (%d/%d)\n",
		requiredPctStyle.Render(fmt.Sprintf("%3d%%", requiredPct)),
		requiredFound, len(required))
	fmt.Fprintf(w, "Optional met: %s (%d/%d)\n",
		optionalPctStyle.Render(fmt.Sprintf("%3d%%", optionalPct)),
		optionalFound, len(optional))

	requiredMissing := len(required) - requiredFound
	optionalMissing := len(optional) - optionalFound

	if requiredMissing > 0 {
		return &process.PassthroughError{Code: 1}
	}
	if strict && optionalMissing > 0 {
		return &process.PassthroughError{Code: 1}
	}

	return nil
}
