package health

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"time"

	"charm.land/lipgloss/v2"

	"devora/internal/config"
	"devora/internal/process"
	"devora/internal/version"
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

type CredentialStatus int

const (
	CredentialOK CredentialStatus = iota
	CredentialFailed
	CredentialUnchecked
)

type CredentialResult struct {
	Name    string
	Status  CredentialStatus
	Message string
}

var getAppVersion = version.Get

var getConfigPath = config.ConfigPath

var statFile = os.Stat

func zshCompletionPath() string {
	return homeDir + "/.zsh/completions/_debi"
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

var checkGitHubToken = defaultCheckGitHubToken

func defaultCheckGitHubToken() bool {
	_, err := process.GetOutput([]string{"gh", "auth", "token"})
	return err == nil
}

var checkGitHub = defaultCheckGitHub

func defaultCheckGitHub() (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, "gh", "api", "user", "--jq", ".name // .login")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		errMsg := strings.TrimSpace(stderr.String())
		if firstLine, _, ok := strings.Cut(errMsg, "\n"); ok {
			errMsg = firstLine
		}
		if errMsg == "" {
			errMsg = err.Error()
		}
		return "", errors.New(errMsg)
	}
	return strings.TrimSpace(stdout.String()), nil
}

func checkCredentials(ghFound bool) []CredentialResult {
	result := CredentialResult{Name: "GitHub"}
	if !ghFound {
		result.Status = CredentialUnchecked
		result.Message = "gh not detected"
	} else {
		hasToken := checkGitHubToken()
		if !hasToken {
			result.Status = CredentialFailed
			result.Message = "no token stored (run: gh auth login)"
		} else {
			displayName, err := checkGitHub()
			if err != nil {
				result.Status = CredentialFailed
				result.Message = err.Error()
			} else {
				result.Status = CredentialOK
				result.Message = fmt.Sprintf("Logged in as %s", displayName)
			}
		}
	}
	return []CredentialResult{result}
}

const ghDepName = "gh"

var dependencies = []Dependency{
	{Name: "kitty", Required: true, VersionCommand: []string{"kitty", "--version"}},
	{Name: "claude", Required: true, VersionCommand: []string{"claude", "--version"}},
	{Name: "git", Required: true, VersionCommand: []string{"git", "--version"}},
	{Name: "uv", Required: true, VersionCommand: []string{"uv", "--version"}},
	{Name: "glow", Required: true, VersionCommand: []string{"glow", "--version"}},
	{Name: "zsh", Required: true, VersionCommand: []string{"zsh", "--version"}},
	{Name: "nvim", Required: false, VersionCommand: []string{"nvim", "--version"}},
	{Name: "mise", Required: false, VersionCommand: []string{"mise", "--version"}},
	{Name: ghDepName, Required: false, VersionCommand: []string{"gh", "--version"}},
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

func renderCredentials(w io.Writer, results []CredentialResult, nameWidth int, greenStyle, redStyle, yellowStyle lipgloss.Style) int {
	ok := 0
	for _, r := range results {
		switch r.Status {
		case CredentialOK:
			ok++
			prefix := greenStyle.Render(fmt.Sprintf("  ✓ %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		case CredentialFailed:
			prefix := redStyle.Render(fmt.Sprintf("  ✗ %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		case CredentialUnchecked:
			prefix := yellowStyle.Render(fmt.Sprintf("  ? %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		}
	}
	return ok
}

func renderSummaryLine(w io.Writer, labelWidth int, label string, found, total int, style lipgloss.Style) {
	pct := 0
	if total > 0 {
		pct = found * 100 / total
	}
	fmt.Fprintf(w, "%-*s %s (%d/%d)\n",
		labelWidth, label,
		style.Render(fmt.Sprintf("%3d%%", pct)),
		found, total)
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
	yellowStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#F9E2AF"))

	fmt.Fprintf(w, "Devora Health Check (version: %s)\n", getAppVersion())
	fmt.Fprintln(w)

	configPath := getConfigPath()
	_, configErr := statFile(configPath)
	if configErr == nil {
		fmt.Fprintf(w, "Config: %s %s\n", shortenPath(configPath), greenStyle.Render("✓"))
	} else {
		fmt.Fprintf(w, "Config: %s %s\n", shortenPath(configPath), yellowStyle.Render("(not found)"))
	}
	fmt.Fprintln(w)

	fmt.Fprintln(w, "Required:")
	requiredFound := renderSection(w, required, nameWidth, versionWidth, verbose, greenStyle, redStyle)

	fmt.Fprintln(w)

	fmt.Fprintln(w, "Optional:")
	optionalFound := renderSection(w, optional, nameWidth, versionWidth, verbose, greenStyle, redStyle)

	// Credential checks
	ghFound := false
	for _, r := range optional {
		if r.Name == ghDepName {
			ghFound = r.Found
			break
		}
	}

	credResults := checkCredentials(ghFound)

	credNameWidth := 0
	for _, r := range credResults {
		if len(r.Name) > credNameWidth {
			credNameWidth = len(r.Name)
		}
	}

	fmt.Fprintln(w)
	fmt.Fprintln(w, "Credentials:")
	credFound := renderCredentials(w, credResults, credNameWidth, greenStyle, redStyle, yellowStyle)

	completionPath := zshCompletionPath()
	_, completionErr := statFile(completionPath)
	completionFound := completionErr == nil
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Completion:")
	if completionFound {
		prefix := greenStyle.Render("  ✓ zsh completion")
		if verbose {
			fmt.Fprintf(w, "%s  %s\n", prefix, shortenPath(completionPath))
		} else {
			fmt.Fprintln(w, prefix)
		}
	} else {
		prefix := redStyle.Render("  ✗ zsh completion")
		fmt.Fprintf(w, "%s  run: debi completion zsh > ~/.zsh/completions/_debi\n", prefix)
	}

	// Summary - align all labels to the longest one
	const (
		requiredLabel    = "Required met:"
		optionalLabel    = "Optional met:"
		credentialsLabel = "Credentials met:"
	)

	summaryLabelWidth := len(credentialsLabel) // longest label

	fmt.Fprintln(w)

	requiredPctStyle := greenStyle
	if requiredFound < len(required) {
		requiredPctStyle = redStyle
	}
	optionalPctStyle := greenStyle
	if optionalFound < len(optional) {
		optionalPctStyle = yellowStyle
	}

	renderSummaryLine(w, summaryLabelWidth, requiredLabel, requiredFound, len(required), requiredPctStyle)
	renderSummaryLine(w, summaryLabelWidth, optionalLabel, optionalFound, len(optional), optionalPctStyle)

	credPctStyle := greenStyle
	if credFound < len(credResults) {
		hasFailed := false
		for _, r := range credResults {
			if r.Status == CredentialFailed {
				hasFailed = true
				break
			}
		}
		if hasFailed {
			credPctStyle = redStyle
		} else {
			credPctStyle = yellowStyle
		}
	}

	renderSummaryLine(w, summaryLabelWidth, credentialsLabel, credFound, len(credResults), credPctStyle)

	requiredMissing := len(required) - requiredFound
	optionalMissing := len(optional) - optionalFound
	credentialsMissing := len(credResults) - credFound

	if requiredMissing > 0 {
		return &process.PassthroughError{Code: 1}
	}
	if strict && (optionalMissing > 0 || credentialsMissing > 0 || !completionFound) {
		return &process.PassthroughError{Code: 1}
	}

	return nil
}
