package health

import (
	"bytes"
	"errors"
	"os"
	"strings"
	"testing"

	"devora/internal/credentials"
	"devora/internal/process"
)

func stubHealthDeps(t *testing.T) {
	t.Helper()
	origLookPath := lookPath
	origGetVersion := getVersion
	origCheckGitHub := checkGitHub
	origCheckGitHubToken := checkGitHubToken
	origGetAppVersion := getAppVersion
	origGetConfigPath := getConfigPath
	origStatFile := statFile
	origGetTrackerProvider := getTrackerProvider
	origGetTrackerToken := getTrackerToken
	t.Cleanup(func() {
		lookPath = origLookPath
		getVersion = origGetVersion
		checkGitHub = origCheckGitHub
		checkGitHubToken = origCheckGitHubToken
		getAppVersion = origGetAppVersion
		getConfigPath = origGetConfigPath
		statFile = origStatFile
		getTrackerProvider = origGetTrackerProvider
		getTrackerToken = origGetTrackerToken
	})
	getAppVersion = func() string { return "test-version" }
	getConfigPath = func() string { return "/home/testuser/.config/devora/config.json" }
	statFile = func(name string) (os.FileInfo, error) { return nil, os.ErrNotExist }
	checkGitHubToken = func() bool { return true }
	// Default: no tracker configured. Tests that want a tracker credential
	// row override these two vars.
	getTrackerProvider = func() string { return "" }
	getTrackerToken = func(provider string) (string, error) {
		t.Fatalf("getTrackerToken should not be called when no tracker is configured")
		return "", nil
	}
}

func TestCleanVersion(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"kitty 0.44.0 created by Kovid Goyal", "0.44.0"},
		{"2.1.89 (Claude Code)", "2.1.89"},
		{"git version 2.50.1 (Apple Git-155)", "2.50.1"},
		{"uv 0.11.2 (Homebrew 2026-03-26 aarch64-apple-darwin)", "0.11.2"},
		{"glow version 2.1.1 (d37e988)", "2.1.1"},
		{"zsh 5.9 (arm64-apple-darwin25.0)", "5.9"},
		{"NVIM v0.12.0", "0.12.0"},
		{"2026.3.17 macos-arm64 (2026-03-27)", "2026.3.17"},
		{"v1.0.0", "1.0.0"},
		{"", ""},
		{"no version here", "no version here"},
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := cleanVersion(tt.input)
			if got != tt.want {
				t.Fatalf("cleanVersion(%q) = %q, want %q", tt.input, got, tt.want)
			}
		})
	}
}

func TestShortenPath(t *testing.T) {
	origHome := homeDir
	t.Cleanup(func() { homeDir = origHome })
	homeDir = "/home/testuser"

	tests := []struct {
		input string
		want  string
	}{
		{"/home/testuser/bin/kitty", "~/bin/kitty"},
		{"/home/testuser", "~"},
		{"/usr/local/bin/git", "/usr/local/bin/git"},
		{"", ""},
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := shortenPath(tt.input)
			if got != tt.want {
				t.Fatalf("shortenPath(%q) = %q, want %q", tt.input, got, tt.want)
			}
		})
	}
}

func TestCheck_FoundDependency(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/kitty", nil
	}
	getVersion = func(command []string) (string, error) {
		return "kitty 0.44.0 created by Kovid Goyal", nil
	}

	dep := Dependency{
		Name:           "kitty",
		Required:       true,
		VersionCommand: []string{"kitty", "--version"},
	}

	result := Check(dep)

	if !result.Found {
		t.Fatal("expected Found to be true")
	}
	if result.Path != "/usr/local/bin/kitty" {
		t.Fatalf("expected Path to be /usr/local/bin/kitty, got: %s", result.Path)
	}
	if result.Version != "0.44.0" {
		t.Fatalf("expected Version to be '0.44.0', got: %s", result.Version)
	}
}

func TestCheck_NotFoundDependency(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "", errors.New("not found")
	}
	getVersion = func(command []string) (string, error) {
		return "", errors.New("should not be called")
	}

	dep := Dependency{
		Name:           "uv",
		Required:       true,
		VersionCommand: []string{"uv", "--version"},
	}

	result := Check(dep)

	if result.Found {
		t.Fatal("expected Found to be false")
	}
	if result.Path != "" {
		t.Fatalf("expected empty Path, got: %s", result.Path)
	}
	if result.Version != "" {
		t.Fatalf("expected empty Version, got: %s", result.Version)
	}
}

func TestCheck_FoundButVersionFails(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/bin/git", nil
	}
	getVersion = func(command []string) (string, error) {
		return "", errors.New("version command failed")
	}

	dep := Dependency{
		Name:           "git",
		Required:       true,
		VersionCommand: []string{"git", "--version"},
	}

	result := Check(dep)

	if !result.Found {
		t.Fatal("expected Found to be true")
	}
	if result.Path != "/usr/bin/git" {
		t.Fatalf("expected Path to be /usr/bin/git, got: %s", result.Path)
	}
	if result.Version != "" {
		t.Fatalf("expected empty Version, got: %s", result.Version)
	}
}

func TestCheck_MultiLineVersionOutput(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/nvim", nil
	}
	getVersion = func(command []string) (string, error) {
		return "NVIM v0.10.0\nBuild type: Release\nLuaJIT 2.1.0", nil
	}

	dep := Dependency{
		Name:           "nvim",
		Required:       false,
		VersionCommand: []string{"nvim", "--version"},
	}

	result := Check(dep)

	if !result.Found {
		t.Fatal("expected Found to be true")
	}
	if result.Version != "0.10.0" {
		t.Fatalf("expected Version to be '0.10.0', got: %s", result.Version)
	}
}

func TestRun_VersionBanner(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	expectedBanner := "Devora Health Check (version: test-version)"
	if !strings.Contains(output, expectedBanner) {
		t.Fatalf("expected output to contain %q, got:\n%s", expectedBanner, output)
	}
}

func TestRun_ConfigFileExists(t *testing.T) {
	stubHealthDeps(t)
	origHome := homeDir
	t.Cleanup(func() { homeDir = origHome })
	homeDir = "/home/testuser"

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	getConfigPath = func() string { return "/home/testuser/.config/devora/config.json" }
	statFile = func(name string) (os.FileInfo, error) { return nil, nil }

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "Config:") {
		t.Fatalf("expected output to contain 'Config:', got:\n%s", output)
	}
	if !strings.Contains(output, "~/.config/devora/config.json") {
		t.Fatalf("expected output to contain shortened config path, got:\n%s", output)
	}
	if !strings.Contains(output, "✓") {
		t.Fatalf("expected output to contain checkmark for existing config, got:\n%s", output)
	}
}

func TestRun_ConfigFileNotFound(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	getConfigPath = func() string { return "/home/testuser/.config/devora/config.json" }
	statFile = func(name string) (os.FileInfo, error) { return nil, os.ErrNotExist }

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "Config:") {
		t.Fatalf("expected output to contain 'Config:', got:\n%s", output)
	}
	if !strings.Contains(output, "(not found)") {
		t.Fatalf("expected output to contain '(not found)' for missing config, got:\n%s", output)
	}
}

func TestRun_ConfigFileNotFound_DoesNotAffectExitCode(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	configPath := "/home/testuser/.config/devora/config.json"
	getConfigPath = func() string { return configPath }
	statFile = func(name string) (os.FileInfo, error) {
		if name == configPath {
			return nil, os.ErrNotExist
		}
		return nil, nil
	}

	var buf bytes.Buffer
	err := Run(&buf, true, false)

	if err != nil {
		t.Fatalf("expected no error even in strict mode with missing config, got: %s", err.Error())
	}
}

func TestRun_AllFound(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) { return nil, nil }

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	for _, dep := range dependencies {
		if !strings.Contains(output, dep.Name) {
			t.Fatalf("expected output to contain %s", dep.Name)
		}
	}
	if strings.Contains(output, "\u2717") {
		t.Fatal("expected no failure markers in output")
	}
	if !strings.Contains(output, "Required:") {
		t.Fatal("expected output to contain 'Required:' header")
	}
	if !strings.Contains(output, "Optional:") {
		t.Fatal("expected output to contain 'Optional:' header")
	}
	if !strings.Contains(output, "Required met:") || !strings.Contains(output, "(6/6)") {
		t.Fatalf("expected required summary line with count, got:\n%s", output)
	}
	if !strings.Contains(output, "Optional met:") || !strings.Contains(output, "(3/3)") {
		t.Fatalf("expected optional summary line with count, got:\n%s", output)
	}
	if !strings.Contains(output, "Credentials:") {
		t.Fatal("expected output to contain 'Credentials:' header")
	}
	if !strings.Contains(output, "Credentials met:") || !strings.Contains(output, "(1/1)") {
		t.Fatalf("expected credentials summary with (1/1), got:\n%s", output)
	}
}

func TestRun_DefaultHidesPath(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if strings.Contains(output, "/usr/local/bin/") {
		t.Fatalf("expected paths to be hidden in non-verbose mode, got:\n%s", output)
	}
}

func TestRun_VerboseShowsPath(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, true)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "/usr/local/bin/kitty") {
		t.Fatalf("expected paths to be shown in verbose mode, got:\n%s", output)
	}
}

func TestRun_RequiredMissing(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		if file == "kitty" {
			return "", errors.New("not found")
		}
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err == nil {
		t.Fatal("expected error when required dependency is missing")
	}
	var passErr *process.PassthroughError
	if !errors.As(err, &passErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", err, err.Error())
	}
	if passErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", passErr.Code)
	}
}

func TestRun_OnlyOptionalMissing(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		if file == "nvim" {
			return "", errors.New("not found")
		}
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error when only optional deps missing, got: %s", err.Error())
	}
}

func TestRun_Strict_OptionalMissing(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		if file == "mise" {
			return "", errors.New("not found")
		}
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, true, false)

	if err == nil {
		t.Fatal("expected error in strict mode when optional dep missing")
	}
	var passErr *process.PassthroughError
	if !errors.As(err, &passErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", err, err.Error())
	}
	if passErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", passErr.Code)
	}
}

func TestRun_Strict_AllFound(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) { return nil, nil }

	var buf bytes.Buffer
	err := Run(&buf, true, false)

	if err != nil {
		t.Fatalf("expected no error in strict mode when all found, got: %s", err.Error())
	}
}

func TestCheckCredentials_GhMissing(t *testing.T) {
	stubHealthDeps(t)
	checkGitHubToken = func() bool {
		t.Fatal("checkGitHubToken should not be called when gh is not found")
		return false
	}
	checkGitHub = func() (string, error) {
		t.Fatal("checkGitHub should not be called when gh is not found")
		return "", nil
	}

	results := checkCredentials(false)

	// Expect GitHub row + tracker info row (unconfigured).
	if len(results) != 2 {
		t.Fatalf("expected 2 results (GitHub + tracker info), got %d", len(results))
	}
	r := results[0]
	if r.Status != CredentialUnchecked {
		t.Fatalf("expected status CredentialUnchecked, got %d", r.Status)
	}
	if r.Message != "gh not detected" {
		t.Fatalf("expected message 'gh not detected', got %q", r.Message)
	}
	if results[1].Status != CredentialInfo {
		t.Fatalf("expected second result to be info row, got status %d", results[1].Status)
	}
}

func TestCheckCredentials_GhFound_NoToken(t *testing.T) {
	stubHealthDeps(t)
	checkGitHubToken = func() bool { return false }
	checkGitHub = func() (string, error) {
		t.Fatal("checkGitHub should not be called when no token is stored")
		return "", nil
	}

	results := checkCredentials(true)

	if len(results) != 2 {
		t.Fatalf("expected 2 results (GitHub + tracker info), got %d", len(results))
	}
	r := results[0]
	if r.Name != "GitHub" {
		t.Fatalf("expected name 'GitHub', got %q", r.Name)
	}
	if r.Status != CredentialFailed {
		t.Fatalf("expected status CredentialFailed, got %d", r.Status)
	}
	if !strings.Contains(r.Message, "no token stored") {
		t.Fatalf("expected message to contain 'no token stored', got %q", r.Message)
	}
	if !strings.Contains(r.Message, "gh auth login") {
		t.Fatalf("expected message to contain 'gh auth login', got %q", r.Message)
	}
	if results[1].Status != CredentialInfo {
		t.Fatalf("expected second result to be info row, got status %d", results[1].Status)
	}
}

func TestCheckCredentials_GhFound_TokenExists_AuthOK(t *testing.T) {
	stubHealthDeps(t)
	checkGitHubToken = func() bool { return true }
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	results := checkCredentials(true)

	if len(results) != 2 {
		t.Fatalf("expected 2 results (GitHub + tracker info), got %d", len(results))
	}
	r := results[0]
	if r.Name != "GitHub" {
		t.Fatalf("expected name 'GitHub', got %q", r.Name)
	}
	if r.Status != CredentialOK {
		t.Fatalf("expected status CredentialOK, got %d", r.Status)
	}
	if r.Message != "Logged in as Test User" {
		t.Fatalf("expected message 'Logged in as Test User', got %q", r.Message)
	}
	if results[1].Status != CredentialInfo {
		t.Fatalf("expected second result to be info row, got status %d", results[1].Status)
	}
}

func TestCheckCredentials_GhFound_TokenExists_AuthFailed(t *testing.T) {
	stubHealthDeps(t)
	checkGitHubToken = func() bool { return true }
	checkGitHub = func() (string, error) {
		return "", errors.New("auth token expired")
	}

	results := checkCredentials(true)

	if len(results) != 2 {
		t.Fatalf("expected 2 results (GitHub + tracker info), got %d", len(results))
	}
	r := results[0]
	if r.Status != CredentialFailed {
		t.Fatalf("expected status CredentialFailed, got %d", r.Status)
	}
	if r.Message != "auth token expired" {
		t.Fatalf("expected message 'auth token expired', got %q", r.Message)
	}
	if results[1].Status != CredentialInfo {
		t.Fatalf("expected second result to be info row, got status %d", results[1].Status)
	}
}

func TestRun_CredentialSection_GhMissing(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		if file == "gh" {
			return "", errors.New("not found")
		}
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		t.Fatal("checkGitHub should not be called when gh is not found")
		return "", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "Credentials:") {
		t.Fatal("expected output to contain 'Credentials:' header")
	}
	if !strings.Contains(output, "gh not detected") {
		t.Fatalf("expected output to contain 'gh not detected', got:\n%s", output)
	}
	if !strings.Contains(output, "?") {
		t.Fatalf("expected output to contain '?' marker for unchecked credential, got:\n%s", output)
	}
}

func TestRun_CredentialSection_GhFound_AuthOK(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "Logged in as Test User") {
		t.Fatalf("expected output to contain 'Logged in as Test User', got:\n%s", output)
	}
	if !strings.Contains(output, "Credentials met:") || !strings.Contains(output, "(1/1)") {
		t.Fatalf("expected credentials summary with (1/1), got:\n%s", output)
	}
}

func TestRun_CredentialSection_GhFound_AuthFailed(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "", errors.New("auth token expired")
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "auth token expired") {
		t.Fatalf("expected output to contain 'auth token expired', got:\n%s", output)
	}
	if !strings.Contains(output, "Credentials met:") || !strings.Contains(output, "(0/1)") {
		t.Fatalf("expected credentials summary with (0/1), got:\n%s", output)
	}
}

func TestRun_CredentialFailure_DoesNotAffectExitCode(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "", errors.New("not authenticated")
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error even with credential failure, got: %s", err.Error())
	}
}

func TestRun_CompletionInstalled(t *testing.T) {
	stubHealthDeps(t)
	origHome := homeDir
	t.Cleanup(func() { homeDir = origHome })
	homeDir = "/home/testuser"

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) {
		if name == "/home/testuser/.zsh/completions/_debi" {
			return nil, nil
		}
		return nil, os.ErrNotExist
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "Completion:") {
		t.Fatalf("expected output to contain 'Completion:', got:\n%s", output)
	}
	if !strings.Contains(output, "zsh completion") {
		t.Fatalf("expected output to contain 'zsh completion', got:\n%s", output)
	}
	if !strings.Contains(output, "✓") {
		t.Fatalf("expected output to contain checkmark for installed completion, got:\n%s", output)
	}
}

func TestRun_CompletionInstalled_StrictExitCodeZero(t *testing.T) {
	stubHealthDeps(t)
	origHome := homeDir
	t.Cleanup(func() { homeDir = origHome })
	homeDir = "/home/testuser"

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) {
		if name == "/home/testuser/.zsh/completions/_debi" {
			return nil, nil
		}
		return nil, os.ErrNotExist
	}

	var buf bytes.Buffer
	err := Run(&buf, true, false)

	if err != nil {
		t.Fatalf("expected no error in strict mode when completion is installed, got: %s", err.Error())
	}
}

func TestRun_CompletionMissing_DefaultModeExitCodeZero(t *testing.T) {
	stubHealthDeps(t)
	origHome := homeDir
	t.Cleanup(func() { homeDir = origHome })
	homeDir = "/home/testuser"

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) { return nil, os.ErrNotExist }

	var buf bytes.Buffer
	err := Run(&buf, false, false)

	if err != nil {
		t.Fatalf("expected no error in default mode when completion missing, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "Completion:") {
		t.Fatalf("expected output to contain 'Completion:', got:\n%s", output)
	}
	if !strings.Contains(output, "✗") {
		t.Fatalf("expected output to contain ✗ for missing completion, got:\n%s", output)
	}
	if !strings.Contains(output, "debi completion zsh > ~/.zsh/completions/_debi") {
		t.Fatalf("expected output to contain install hint, got:\n%s", output)
	}
}

func TestRun_CompletionMissing_StrictModeExitCode1(t *testing.T) {
	stubHealthDeps(t)
	origHome := homeDir
	t.Cleanup(func() { homeDir = origHome })
	homeDir = "/home/testuser"

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) { return nil, os.ErrNotExist }

	var buf bytes.Buffer
	err := Run(&buf, true, false)

	if err == nil {
		t.Fatal("expected error in strict mode when completion missing")
	}
	var passErr *process.PassthroughError
	if !errors.As(err, &passErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", err, err.Error())
	}
	if passErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", passErr.Code)
	}
}

func TestCheckTrackerCredential_NoTrackerConfigured_ReturnsInfoRow(t *testing.T) {
	stubHealthDeps(t)
	// Default stubbed getTrackerProvider returns "".

	got := checkTrackerCredential()
	if got == nil {
		t.Fatal("expected an info row when no tracker is configured, got nil")
	}
	if got.Status != CredentialInfo {
		t.Fatalf("expected CredentialInfo status, got %d", got.Status)
	}
	if got.Name != "task-tracker" {
		t.Fatalf("expected name 'task-tracker', got %q", got.Name)
	}
	if !strings.Contains(got.Message, "not configured") {
		t.Fatalf("expected message to indicate 'not configured', got %q", got.Message)
	}
	if !strings.Contains(got.Message, "(optional)") {
		t.Fatalf("expected message to indicate '(optional)', got %q", got.Message)
	}
}

func TestCheckTrackerCredential_TrackerConfiguredTokenStored_ReturnsOK(t *testing.T) {
	stubHealthDeps(t)
	getTrackerProvider = func() string { return "asana" }
	getTrackerToken = func(provider string) (string, error) {
		if provider != "asana" {
			t.Fatalf("expected provider 'asana', got %q", provider)
		}
		return "tok_xyz", nil
	}

	got := checkTrackerCredential()
	if got == nil {
		t.Fatal("expected a result, got nil")
	}
	if got.Status != CredentialOK {
		t.Fatalf("expected CredentialOK, got %d", got.Status)
	}
	if !strings.Contains(got.Name, "asana") {
		t.Fatalf("expected name to include provider, got %q", got.Name)
	}
	if got.Message == "" {
		t.Fatal("expected non-empty message")
	}
}

func TestCheckTrackerCredential_TrackerConfiguredTokenMissing_ReturnsFailedWithHint(t *testing.T) {
	stubHealthDeps(t)
	getTrackerProvider = func() string { return "asana" }
	getTrackerToken = func(provider string) (string, error) {
		return "", &credentials.NotFoundError{Provider: "asana", Service: "devora-asana", Account: "alice"}
	}

	got := checkTrackerCredential()
	if got == nil {
		t.Fatal("expected a result, got nil")
	}
	if got.Status != CredentialFailed {
		t.Fatalf("expected CredentialFailed, got %d", got.Status)
	}
	// The message should contain a setup hint — on macOS this includes
	// "security add-generic-password"; on linux it's "secret-tool"; on
	// windows it's "cmdkey"; fall back to "OS keychain" text.
	msg := got.Message
	if !strings.Contains(msg, "security add-generic-password") &&
		!strings.Contains(msg, "secret-tool") &&
		!strings.Contains(msg, "cmdkey") &&
		!strings.Contains(msg, "OS keychain") {
		t.Fatalf("expected message to contain a setup hint, got: %q", msg)
	}
}

func TestCheckTrackerCredential_TrackerConfiguredOtherError_ReturnsFailedWithError(t *testing.T) {
	stubHealthDeps(t)
	getTrackerProvider = func() string { return "asana" }
	getTrackerToken = func(provider string) (string, error) {
		return "", errors.New("keychain access timed out")
	}

	got := checkTrackerCredential()
	if got == nil {
		t.Fatal("expected a result, got nil")
	}
	if got.Status != CredentialFailed {
		t.Fatalf("expected CredentialFailed, got %d", got.Status)
	}
	if !strings.Contains(got.Message, "timed out") {
		t.Fatalf("expected message to surface the error, got: %q", got.Message)
	}
}

func TestRun_NoTracker_ShowsInfoRow(t *testing.T) {
	// When no tracker is configured, health renders an info row so users
	// can see the feature exists, but the row does NOT count toward the
	// "Credentials met: X/Y" denominator. With just GitHub counting, the
	// summary should read (1/1).
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	// Default stub: getTrackerProvider returns "" (no tracker configured).

	var buf bytes.Buffer
	err := Run(&buf, false, false)
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "task-tracker") {
		t.Fatalf("expected output to contain 'task-tracker' info row, got:\n%s", output)
	}
	if !strings.Contains(output, "not configured") {
		t.Fatalf("expected output to contain 'not configured', got:\n%s", output)
	}
	if !strings.Contains(output, "\u25CB") {
		t.Fatalf("expected output to contain ○ marker for info row, got:\n%s", output)
	}
	if !strings.Contains(output, "Credentials met:") || !strings.Contains(output, "(1/1)") {
		t.Fatalf("expected credentials summary (1/1) excluding info row, got:\n%s", output)
	}
}

func TestRun_NoTracker_Strict_InfoRowDoesNotFailStrict(t *testing.T) {
	// Info rows are not "missing" — strict mode should pass when the only
	// non-OK credential row is an info row.
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	statFile = func(name string) (os.FileInfo, error) { return nil, nil }

	var buf bytes.Buffer
	err := Run(&buf, true, false)
	if err != nil {
		t.Fatalf("expected no error in strict mode with tracker info row, got: %s", err.Error())
	}
}

func TestRun_TrackerConfigured_AddsTrackerRow(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	getTrackerProvider = func() string { return "asana" }
	getTrackerToken = func(provider string) (string, error) { return "tok", nil }

	var buf bytes.Buffer
	err := Run(&buf, false, false)
	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "asana") {
		t.Fatalf("expected output to contain 'asana' tracker row, got:\n%s", output)
	}
	if !strings.Contains(output, "(2/2)") {
		t.Fatalf("expected credentials summary (2/2), got:\n%s", output)
	}
}

func TestRun_TrackerConfigured_TokenMissing_CredentialsRow(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}
	getTrackerProvider = func() string { return "asana" }
	getTrackerToken = func(provider string) (string, error) {
		return "", &credentials.NotFoundError{Provider: "asana", Service: "devora-asana", Account: "alice"}
	}

	var buf bytes.Buffer
	err := Run(&buf, false, false)
	if err != nil {
		t.Fatalf("expected no error (non-strict, credential failure), got: %s", err.Error())
	}
	output := buf.String()
	if !strings.Contains(output, "asana") {
		t.Fatalf("expected output to contain 'asana' tracker row, got:\n%s", output)
	}
	if !strings.Contains(output, "\u2717") {
		t.Fatalf("expected ✗ marker for missing tracker token, got:\n%s", output)
	}
	if !strings.Contains(output, "(1/2)") {
		t.Fatalf("expected credentials summary (1/2), got:\n%s", output)
	}
}

func TestRun_Strict_CredentialFailure_CausesExitCode1(t *testing.T) {
	stubHealthDeps(t)

	lookPath = func(file string) (string, error) {
		return "/usr/local/bin/" + file, nil
	}
	getVersion = func(command []string) (string, error) {
		return "v1.0.0", nil
	}
	checkGitHub = func() (string, error) {
		return "", errors.New("not authenticated")
	}

	var buf bytes.Buffer
	err := Run(&buf, true, false)

	if err == nil {
		t.Fatal("expected error in strict mode when credential check fails")
	}
	var passErr *process.PassthroughError
	if !errors.As(err, &passErr) {
		t.Fatalf("expected PassthroughError, got %T: %s", err, err.Error())
	}
	if passErr.Code != 1 {
		t.Fatalf("expected exit code 1, got: %d", passErr.Code)
	}
}
