package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"devora/internal/config"
	"devora/internal/git"
)

// autoMergeKey is the git config key used to store the per-repo auto-merge
// override. Duplicated here from internal/submit's adapter so the CLI layer
// does not depend on the submit package just to reuse a literal.
const autoMergeKey = "devora.pr.auto-merge"

// Package-level stubbable vars let auto_merge_test.go exercise the handler
// without touching the filesystem, the active profile, or git state.
var (
	setPrAutoMergeGlobal  = config.SetPrAutoMergeGlobal
	setPrAutoMergeProfile = config.SetPrAutoMergeProfile
	getPrAutoMergeGlobal  = config.GetPrAutoMergeGlobalRaw
	getPrAutoMergeProfile = config.GetPrAutoMergeProfileRaw
	setRepoAutoMerge      = func(v bool) error { return git.SetRepoConfigBool(autoMergeKey, v) }
	unsetRepoAutoMerge    = func() error { return git.UnsetRepoConfig(autoMergeKey) }
	getRepoAutoMergeRaw   = func() (*bool, error) { return git.GetRepoConfigBool(autoMergeKey) }
)

const autoMergeUsage = `usage: debi pr auto-merge <verb> [--scope=repo|profile|global] [--json]

Configure the auto-merge default for "debi pr submit". Precedence
(highest wins): per-repo > profile > global > built-in default (on).

Verbs:
  enable    Set the value to true at the chosen scope
  disable   Set the value to false at the chosen scope
  reset     Remove the value at the chosen scope (idempotent)
  show      Print the resolved value and each layer's contribution

Flags:
  --scope <repo|profile|global>   Target scope (default: repo)
  --json                          For "show", emit JSON on stdout
  -h, --help                      Show this help`

// autoMergeArgs are the parsed flags for `debi pr auto-merge`.
type autoMergeArgs struct {
	verb       string
	scope      string
	jsonOutput bool
}

// parseAutoMergeArgs walks args and returns either a populated autoMergeArgs,
// a help-request flag, or a *UsageError. Scope defaults to "repo"; supports
// both --scope=val and --scope val syntaxes.
func parseAutoMergeArgs(args []string) (autoMergeArgs, bool, error) {
	parsed := autoMergeArgs{scope: "repo"}

	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "-h" || arg == "--help":
			return parsed, true, nil
		case arg == "--json":
			parsed.jsonOutput = true
		case arg == "--scope" || strings.HasPrefix(arg, "--scope="):
			val, nextI, err := parseValue(args, i, arg, "--scope")
			if err != nil {
				return parsed, false, err
			}
			parsed.scope = val
			i = nextI
		case strings.HasPrefix(arg, "-"):
			return parsed, false, &UsageError{Message: fmt.Sprintf("unknown flag: %s\n%s", arg, autoMergeUsage)}
		default:
			if parsed.verb != "" {
				return parsed, false, &UsageError{Message: fmt.Sprintf("unexpected argument: %s\n%s", arg, autoMergeUsage)}
			}
			parsed.verb = arg
		}
	}

	if parsed.verb == "" {
		return parsed, false, &UsageError{Message: "auto-merge requires a verb\n" + autoMergeUsage}
	}
	switch parsed.verb {
	case "enable", "disable", "reset", "show":
		// valid
	default:
		return parsed, false, &UsageError{Message: fmt.Sprintf("unknown verb: %s\n%s", parsed.verb, autoMergeUsage)}
	}
	switch parsed.scope {
	case "repo", "profile", "global":
		// valid
	default:
		return parsed, false, &UsageError{Message: fmt.Sprintf("invalid scope: %s (expected repo|profile|global)\n%s", parsed.scope, autoMergeUsage)}
	}
	return parsed, false, nil
}

// runPRAutoMerge is the entry point for `debi pr auto-merge <verb> ...`.
func runPRAutoMerge(args []string) error {
	parsed, helpRequested, err := parseAutoMergeArgs(args)
	if err != nil {
		return err
	}
	if helpRequested {
		fmt.Println(autoMergeUsage)
		return nil
	}

	if parsed.jsonOutput && parsed.verb != "show" {
		return &UsageError{Message: "--json is only valid with the show verb; drop --json or use `debi pr auto-merge show --json`"}
	}

	switch parsed.verb {
	case "enable":
		return runAutoMergeSet(parsed.scope, true)
	case "disable":
		return runAutoMergeSet(parsed.scope, false)
	case "reset":
		return runAutoMergeReset(parsed.scope)
	case "show":
		return runAutoMergeShow(parsed.jsonOutput)
	}
	// Unreachable: parseAutoMergeArgs rejects unknown verbs.
	return &UsageError{Message: "unknown verb"}
}

// runAutoMergeSet applies enable/disable at the given scope.
func runAutoMergeSet(scope string, value bool) error {
	switch scope {
	case "repo":
		if err := git.EnsureInRepo(); err != nil {
			if wrapped, ok := handleNotInGitRepo(err); ok {
				return wrapped
			}
			return err
		}
		if err := setRepoAutoMerge(value); err != nil {
			return err
		}
		literal := boolLiteral(value)
		fmt.Printf("%s auto-merge %s for this clone\n  (git config --local devora.pr.auto-merge=%s)\n",
			"\u2713", verbAdjective(value), literal)
		return nil
	case "profile":
		profileName, err := resolveActiveProfile("")
		if err != nil {
			return err
		}
		if profileName == "" {
			return &UsageError{Message: "no active profile: run this command from a profile directory or register one first"}
		}
		if err := setPrAutoMergeProfile(&value); err != nil {
			return err
		}
		literal := boolLiteral(value)
		fmt.Printf("%s auto-merge %s for profile %q\n  (config.json: pr.auto-merge=%s)\n",
			"\u2713", verbAdjective(value), profileName, literal)
		return nil
	case "global":
		if err := setPrAutoMergeGlobal(&value); err != nil {
			return err
		}
		literal := boolLiteral(value)
		fmt.Printf("%s auto-merge %s globally\n  (~/.config/devora/config.json: pr.auto-merge=%s)\n",
			"\u2713", verbAdjective(value), literal)
		return nil
	}
	return fmt.Errorf("unreachable: invalid scope %q", scope)
}

// runAutoMergeReset clears the value at the given scope. Idempotent: prints
// "cleared ..." when the value existed and "already unset ..." when it did
// not.
func runAutoMergeReset(scope string) error {
	switch scope {
	case "repo":
		if err := git.EnsureInRepo(); err != nil {
			if wrapped, ok := handleNotInGitRepo(err); ok {
				return wrapped
			}
			return err
		}
		existing, err := getRepoAutoMergeRaw()
		if err != nil {
			return err
		}
		if err := unsetRepoAutoMerge(); err != nil {
			return err
		}
		if existing != nil {
			fmt.Println("\u2713 cleared per-repo auto-merge\n  (git config --local --unset devora.pr.auto-merge)")
		} else {
			fmt.Println("\u2713 already unset for this clone\n  (git config --local devora.pr.auto-merge)")
		}
		return nil
	case "profile":
		profileName, err := resolveActiveProfile("")
		if err != nil {
			return err
		}
		if profileName == "" {
			return &UsageError{Message: "no active profile: run this command from a profile directory or register one first"}
		}
		existing := getPrAutoMergeProfile()
		if err := setPrAutoMergeProfile(nil); err != nil {
			return err
		}
		if existing != nil {
			fmt.Printf("\u2713 cleared auto-merge for profile %q\n  (config.json: pr.auto-merge)\n", profileName)
		} else {
			fmt.Printf("\u2713 already unset for profile %q\n  (config.json: pr.auto-merge)\n", profileName)
		}
		return nil
	case "global":
		existing := getPrAutoMergeGlobal()
		if err := setPrAutoMergeGlobal(nil); err != nil {
			return err
		}
		if existing != nil {
			fmt.Println("\u2713 cleared global auto-merge\n  (~/.config/devora/config.json: pr.auto-merge)")
		} else {
			fmt.Println("\u2713 already unset globally\n  (~/.config/devora/config.json: pr.auto-merge)")
		}
		return nil
	}
	return fmt.Errorf("unreachable: invalid scope %q", scope)
}

// autoMergeShowJSON mirrors the user-approved JSON shape emitted by
// `pr auto-merge show --json`.
type autoMergeShowJSON struct {
	Key       string                 `json:"key"`
	Effective bool                   `json:"effective"`
	Source    string                 `json:"source"`
	Layers    autoMergeShowJSONLayer `json:"layers"`
}

type autoMergeShowJSONLayer struct {
	Repo    *bool `json:"repo"`
	Profile *bool `json:"profile"`
	Global  *bool `json:"global"`
	Default bool  `json:"default"`
}

// runAutoMergeShow prints a per-layer breakdown of pr.auto-merge in either
// human-readable or JSON form. Works even outside a git repo or profile;
// missing layers are rendered as <unset> / null.
func runAutoMergeShow(jsonOutput bool) error {
	// Best-effort profile resolution — failures fall through to unset.
	_, _ = resolveActiveProfile("")

	var repo *bool
	if err := git.EnsureInRepo(); err == nil {
		val, rerr := getRepoAutoMergeRaw()
		if rerr != nil {
			fmt.Fprintf(os.Stderr, "warning: failed to read per-repo pr.auto-merge: %v\n", rerr)
		} else {
			repo = val
		}
	}

	profile := getPrAutoMergeProfile()
	global := getPrAutoMergeGlobal()

	effective, source := resolveAutoMergeEffective(repo, profile, global)

	if jsonOutput {
		payload := autoMergeShowJSON{
			Key:       "pr.auto-merge",
			Effective: effective,
			Source:    source,
			Layers: autoMergeShowJSONLayer{
				Repo:    repo,
				Profile: profile,
				Global:  global,
				Default: true,
			},
		}
		return json.NewEncoder(os.Stdout).Encode(payload)
	}

	fmt.Println("pr.auto-merge")
	fmt.Printf("  effective: %s  (from: %s)\n", boolLiteral(effective), source)
	fmt.Printf("  repo:      %s\n", formatOptionalBool(repo))
	fmt.Printf("  profile:   %s\n", formatOptionalBool(profile))
	fmt.Printf("  global:    %s\n", formatOptionalBool(global))
	fmt.Println("  default:   true")
	return nil
}

// resolveAutoMergeEffective picks the first non-nil layer (repo > profile >
// global) and returns the layer name. When all are nil, returns the hard
// default (true) with source "default".
func resolveAutoMergeEffective(repo, profile, global *bool) (bool, string) {
	if repo != nil {
		return *repo, "repo"
	}
	if profile != nil {
		return *profile, "profile"
	}
	if global != nil {
		return *global, "global"
	}
	return true, "default"
}

// boolLiteral renders a bool as the JSON literal "true" or "false". Used in
// human confirmations so the output mirrors the on-disk representation.
func boolLiteral(v bool) string {
	if v {
		return "true"
	}
	return "false"
}

// verbAdjective renders "enabled" / "disabled" for confirmation output.
func verbAdjective(v bool) string {
	if v {
		return "enabled"
	}
	return "disabled"
}

// formatOptionalBool renders a *bool as "true", "false", or "<unset>". Used
// by the human-readable `show` output.
func formatOptionalBool(v *bool) string {
	if v == nil {
		return "<unset>"
	}
	return boolLiteral(*v)
}
