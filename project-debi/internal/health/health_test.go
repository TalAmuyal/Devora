package health

import (
	"bytes"
	"errors"
	"os"
	"strings"
	"testing"

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
	t.Cleanup(func() {
		lookPath = origLookPath
		getVersion = origGetVersion
		checkGitHub = origCheckGitHub
		checkGitHubToken = origCheckGitHubToken
		getAppVersion = origGetAppVersion
		getConfigPath = origGetConfigPath
		statFile = origStatFile
	})
	getAppVersion = func() string { return "test-version" }
	getConfigPath = func() string { return "/home/testuser/.config/devora/config.json" }
	statFile = func(name string) (os.FileInfo, error) { return nil, os.ErrNotExist }
	checkGitHubToken = func() bool { return true }
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
	getConfigPath = func() string { return "/home/testuser/.config/devora/config.json" }
	statFile = func(name string) (os.FileInfo, error) { return nil, os.ErrNotExist }

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

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}
	r := results[0]
	if r.Status != CredentialUnchecked {
		t.Fatalf("expected status CredentialUnchecked, got %d", r.Status)
	}
	if r.Message != "gh not detected" {
		t.Fatalf("expected message 'gh not detected', got %q", r.Message)
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

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
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
}

func TestCheckCredentials_GhFound_TokenExists_AuthOK(t *testing.T) {
	stubHealthDeps(t)
	checkGitHubToken = func() bool { return true }
	checkGitHub = func() (string, error) {
		return "Test User", nil
	}

	results := checkCredentials(true)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
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
}

func TestCheckCredentials_GhFound_TokenExists_AuthFailed(t *testing.T) {
	stubHealthDeps(t)
	checkGitHubToken = func() bool { return true }
	checkGitHub = func() (string, error) {
		return "", errors.New("auth token expired")
	}

	results := checkCredentials(true)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}
	r := results[0]
	if r.Status != CredentialFailed {
		t.Fatalf("expected status CredentialFailed, got %d", r.Status)
	}
	if r.Message != "auth token expired" {
		t.Fatalf("expected message 'auth token expired', got %q", r.Message)
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
