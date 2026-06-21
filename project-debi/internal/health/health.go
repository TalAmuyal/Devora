package health

import (
	"bytes"
	"context"
	"encoding/json"
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
	"devora/internal/credentials"
	"devora/internal/process"
	"devora/internal/style"
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
	// CredentialInfo marks an informational row that doesn't count toward the
	// "Credentials met: X/Y" denominator (e.g., an optional tracker that
	// is not configured). Rendered with a neutral ○ marker.
	CredentialInfo
)

// JSON status strings for CredentialReport.Status, consumed by the Ember UI.
const (
	credStatusOK        = "ok"
	credStatusFailed    = "failed"
	credStatusUnchecked = "unchecked"
	credStatusInfo      = "info"
)

type CredentialResult struct {
	Name    string
	Status  CredentialStatus
	Message string
	// FixHint is a bare, copy-pasteable command that resolves the issue, when one exists (e.g. "gh auth login").
	// Empty when there is no single command.
	FixHint string
}

// --- Structured report (the JSON shape Ember's Health Hub renders) ---

// DependencyStatus is one dependency row.
type DependencyStatus struct {
	Name    string `json:"name"`
	Found   bool   `json:"found"`
	Version string `json:"version"`
	Path    string `json:"path"`
}

// CredentialReport is one credential row. Status is one of credStatus*.
type CredentialReport struct {
	Name    string `json:"name"`
	Status  string `json:"status"`
	Message string `json:"message"`
	FixHint string `json:"fixHint,omitempty"`
}

// FileCheck is a presence check for a known file (config, zsh completion).
type FileCheck struct {
	Path    string `json:"path"`
	Found   bool   `json:"found"`
	FixHint string `json:"fixHint,omitempty"`
}

// Summary holds the met/total counts rendered as percentages.
type Summary struct {
	RequiredMet      int `json:"requiredMet"`
	RequiredTotal    int `json:"requiredTotal"`
	OptionalMet      int `json:"optionalMet"`
	OptionalTotal    int `json:"optionalTotal"`
	CredentialsMet   int `json:"credentialsMet"`
	CredentialsTotal int `json:"credentialsTotal"`
}

// Report is the full structured health result.
// Paths are absolute; the text renderer shortens them for display.
type Report struct {
	Version     string             `json:"version"`
	Config      FileCheck          `json:"config"`
	Completion  FileCheck          `json:"completion"`
	Required    []DependencyStatus `json:"required"`
	Optional    []DependencyStatus `json:"optional"`
	Credentials []CredentialReport `json:"credentials"`
	Summary     Summary            `json:"summary"`
}

var getAppVersion = version.Get

var getConfigPath = config.ConfigPath

var statFile = os.Stat

func zshCompletionPath() string {
	return homeDir + "/.zsh/completions/_debi"
}

const completionFixHint = "debi completion zsh > ~/.zsh/completions/_debi"

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
			result.FixHint = "gh auth login"
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
	results := []CredentialResult{result}
	results = append(results, *checkTrackerCredential())
	return results
}

// Stubbable in tests so we can exercise the tracker-credential branches
// without touching config or the OS keychain.
var (
	getTrackerProvider = config.GetTaskTrackerProvider
	getTrackerToken    = credentials.GetToken
)

// checkTrackerCredential returns a credential row for the task-tracker. When
// no tracker is configured, returns a neutral info row so users can see that
// the feature exists but is unconfigured. When a tracker is configured but
// the token is missing, the message points the user at credentials.SetupHint
// so they know how to store one.
func checkTrackerCredential() *CredentialResult {
	provider := getTrackerProvider()
	if provider == "" {
		return &CredentialResult{
			Name:    "task-tracker",
			Status:  CredentialInfo,
			Message: "not configured (optional)",
		}
	}
	result := CredentialResult{Name: provider + " token"}
	token, err := getTrackerToken(provider)
	if err != nil {
		var nfe *credentials.NotFoundError
		if errors.As(err, &nfe) {
			result.Status = CredentialFailed
			result.Message = credentials.SetupHint(provider)
			return &result
		}
		result.Status = CredentialFailed
		result.Message = err.Error()
		return &result
	}
	if token == "" {
		// GetToken shouldn't return empty on success, but guard anyway.
		result.Status = CredentialFailed
		result.Message = credentials.SetupHint(provider)
		return &result
	}
	result.Status = CredentialOK
	result.Message = "stored in keychain"
	return &result
}

const ghDepName = "gh"

var dependencies = []Dependency{
	{Name: "claude", Required: true, VersionCommand: []string{"claude", "--version"}},
	{Name: "git", Required: true, VersionCommand: []string{"git", "--version"}},
	{Name: "uv", Required: true, VersionCommand: []string{"uv", "--version"}},
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

// countedCredentials returns the number of credential rows that count toward the "Credentials met: X/Y" denominator.
// Info rows (e.g., unconfigured optional tracker) are excluded because they can't be "met" or "not met".
func countedCredentials(results []CredentialResult) int {
	n := 0
	for _, r := range results {
		if r.Status != CredentialInfo {
			n++
		}
	}
	return n
}

func credStatusString(s CredentialStatus) string {
	switch s {
	case CredentialOK:
		return credStatusOK
	case CredentialFailed:
		return credStatusFailed
	case CredentialUnchecked:
		return credStatusUnchecked
	case CredentialInfo:
		return credStatusInfo
	default:
		return ""
	}
}

// Gather runs every health check and returns the structured result.
// It is the single source of truth for both the text renderer (Run) and the JSON output (RunJSON / the Ember Health Hub).
func Gather() Report {
	var required, optional []DependencyStatus
	requiredMet := 0
	optionalMet := 0
	ghFound := false

	for _, dep := range dependencies {
		result := Check(dep)
		row := DependencyStatus{
			Name:    dep.Name,
			Found:   result.Found,
			Version: result.Version,
			Path:    result.Path,
		}
		if dep.Required {
			required = append(required, row)
			if result.Found {
				requiredMet++
			}
		} else {
			optional = append(optional, row)
			if result.Found {
				optionalMet++
			}
			if dep.Name == ghDepName {
				ghFound = result.Found
			}
		}
	}

	credResults := checkCredentials(ghFound)
	creds := make([]CredentialReport, 0, len(credResults))
	credMet := 0
	for _, r := range credResults {
		creds = append(creds, CredentialReport{
			Name:    r.Name,
			Status:  credStatusString(r.Status),
			Message: r.Message,
			FixHint: r.FixHint,
		})
		if r.Status == CredentialOK {
			credMet++
		}
	}
	credTotal := countedCredentials(credResults)

	configPath := getConfigPath()
	_, configErr := statFile(configPath)

	completionPath := zshCompletionPath()
	_, completionErr := statFile(completionPath)
	completion := FileCheck{Path: completionPath, Found: completionErr == nil}
	if !completion.Found {
		completion.FixHint = completionFixHint
	}

	return Report{
		Version:     getAppVersion(),
		Config:      FileCheck{Path: configPath, Found: configErr == nil},
		Completion:  completion,
		Required:    required,
		Optional:    optional,
		Credentials: creds,
		Summary: Summary{
			RequiredMet:      requiredMet,
			RequiredTotal:    len(required),
			OptionalMet:      optionalMet,
			OptionalTotal:    len(optional),
			CredentialsMet:   credMet,
			CredentialsTotal: credTotal,
		},
	}
}

// RunJSON writes the structured report as indented JSON.
// Output is pure JSON (no styling), so `debi health --json | jq` works.
func RunJSON(w io.Writer) error {
	data, err := json.MarshalIndent(Gather(), "", "  ")
	if err != nil {
		return err
	}
	_, err = fmt.Fprintln(w, string(data))
	return err
}

func renderDepSection(w io.Writer, deps []DependencyStatus, nameWidth, versionWidth int, verbose bool) {
	for _, r := range deps {
		if r.Found {
			prefix := style.Success.Render(fmt.Sprintf("  ✓ %-*s", nameWidth, r.Name))
			if verbose {
				fmt.Fprintf(w, "%s  %-*s  %s\n", prefix, versionWidth, r.Version, shortenPath(r.Path))
			} else {
				fmt.Fprintf(w, "%s  %s\n", prefix, r.Version)
			}
		} else {
			prefix := style.Error.Render(fmt.Sprintf("  ✗ %-*s", nameWidth, r.Name))
			if verbose {
				fmt.Fprintf(w, "%s  %-*s  not found\n", prefix, versionWidth, "")
			} else {
				fmt.Fprintf(w, "%s  not found\n", prefix)
			}
		}
	}
}

func renderCredentialRows(w io.Writer, results []CredentialReport, nameWidth int) {
	for _, r := range results {
		switch r.Status {
		case credStatusOK:
			prefix := style.Success.Render(fmt.Sprintf("  ✓ %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		case credStatusFailed:
			prefix := style.Error.Render(fmt.Sprintf("  ✗ %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		case credStatusUnchecked:
			prefix := style.Warning.Render(fmt.Sprintf("  ? %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		case credStatusInfo:
			prefix := style.Muted.Render(fmt.Sprintf("  ○ %-*s", nameWidth, r.Name))
			fmt.Fprintf(w, "%s  %s\n", prefix, r.Message)
		}
	}
}

func renderSummaryLine(w io.Writer, labelWidth int, label string, found, total int, pctStyle lipgloss.Style) {
	pct := 0
	if total > 0 {
		pct = found * 100 / total
	}
	fmt.Fprintf(w, "%-*s %s (%d/%d)\n",
		labelWidth, label,
		pctStyle.Render(fmt.Sprintf("%3d%%", pct)),
		found, total)
}

func Run(w io.Writer, strict bool, verbose bool) error {
	report := Gather()

	nameWidth := 0
	versionWidth := 0
	for _, r := range report.Required {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
		if len(r.Version) > versionWidth {
			versionWidth = len(r.Version)
		}
	}
	for _, r := range report.Optional {
		if len(r.Name) > nameWidth {
			nameWidth = len(r.Name)
		}
		if len(r.Version) > versionWidth {
			versionWidth = len(r.Version)
		}
	}

	fmt.Fprintf(w, "Devora Health Check (version: %s)\n", report.Version)
	fmt.Fprintln(w)

	if report.Config.Found {
		fmt.Fprintf(w, "Config: %s %s\n", shortenPath(report.Config.Path), style.Success.Render("✓"))
	} else {
		fmt.Fprintf(w, "Config: %s %s\n", shortenPath(report.Config.Path), style.Warning.Render("(not found)"))
	}
	fmt.Fprintln(w)

	fmt.Fprintln(w, "Required:")
	renderDepSection(w, report.Required, nameWidth, versionWidth, verbose)

	fmt.Fprintln(w)

	fmt.Fprintln(w, "Optional:")
	renderDepSection(w, report.Optional, nameWidth, versionWidth, verbose)

	credNameWidth := 0
	for _, r := range report.Credentials {
		if len(r.Name) > credNameWidth {
			credNameWidth = len(r.Name)
		}
	}

	fmt.Fprintln(w)
	fmt.Fprintln(w, "Credentials:")
	renderCredentialRows(w, report.Credentials, credNameWidth)

	fmt.Fprintln(w)
	fmt.Fprintln(w, "Completion:")
	if report.Completion.Found {
		prefix := style.Success.Render("  ✓ zsh completion")
		if verbose {
			fmt.Fprintf(w, "%s  %s\n", prefix, shortenPath(report.Completion.Path))
		} else {
			fmt.Fprintln(w, prefix)
		}
	} else {
		prefix := style.Error.Render("  ✗ zsh completion")
		fmt.Fprintf(w, "%s  run: %s\n", prefix, completionFixHint)
	}

	// Summary - align all labels to the longest one
	const (
		requiredLabel    = "Required met:"
		optionalLabel    = "Optional met:"
		credentialsLabel = "Credentials met:"
	)

	summaryLabelWidth := len(credentialsLabel) // longest label

	fmt.Fprintln(w)

	requiredPctStyle := style.Success
	if report.Summary.RequiredMet < report.Summary.RequiredTotal {
		requiredPctStyle = style.Error
	}
	optionalPctStyle := style.Success
	if report.Summary.OptionalMet < report.Summary.OptionalTotal {
		optionalPctStyle = style.Warning
	}

	renderSummaryLine(w, summaryLabelWidth, requiredLabel, report.Summary.RequiredMet, report.Summary.RequiredTotal, requiredPctStyle)
	renderSummaryLine(w, summaryLabelWidth, optionalLabel, report.Summary.OptionalMet, report.Summary.OptionalTotal, optionalPctStyle)

	credPctStyle := style.Success
	if report.Summary.CredentialsMet < report.Summary.CredentialsTotal {
		hasFailed := false
		for _, r := range report.Credentials {
			if r.Status == credStatusFailed {
				hasFailed = true
				break
			}
		}
		if hasFailed {
			credPctStyle = style.Error
		} else {
			credPctStyle = style.Warning
		}
	}

	renderSummaryLine(w, summaryLabelWidth, credentialsLabel, report.Summary.CredentialsMet, report.Summary.CredentialsTotal, credPctStyle)

	requiredMissing := report.Summary.RequiredTotal - report.Summary.RequiredMet
	optionalMissing := report.Summary.OptionalTotal - report.Summary.OptionalMet
	credentialsMissing := report.Summary.CredentialsTotal - report.Summary.CredentialsMet

	if requiredMissing > 0 {
		return &process.PassthroughError{Code: 1}
	}
	if strict && (optionalMissing > 0 || credentialsMissing > 0 || !report.Completion.Found) {
		return &process.PassthroughError{Code: 1}
	}

	return nil
}
