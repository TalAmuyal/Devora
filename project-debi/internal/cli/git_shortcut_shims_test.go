package cli

import (
	"sort"
	"strings"
	"testing"

	"devora/internal/gitcmd"
	"devora/internal/shellinit"
)

// frozenShimNames is the contract this package has with the outside world: Judge's command allowlist contains "gaac" and Ember's e2e suite probes `command -v gcl`, and users' muscle memory relies on all of them.
// The list is duplicated here ON PURPOSE — deriving it from GitShimAliases would make the test blind to renames.
// Additions are fine; renames/removals must be coordinated with Judge and the Ember e2e suite.
var frozenShimNames = []string{
	"gaa",
	"gaaa",
	"gaaap",
	"gaac",
	"gaacp",
	"gb",
	"gbd",
	"gbdc",
	"gcl",
	"gcom",
	"gd",
	"gfo",
	"gg",
	"gl",
	"gpo",
	"gpof",
	"gpop",
	"gri",
	"grl",
	"grlp",
	"grom",
	"gst",
	"gstash",
	"gsum",
}

// passthroughFirstTokens are the git subcommands that shim targets may start with when they are not gitcmd customs
var passthroughFirstTokens = map[string]bool{
	"add":    true,
	"branch": true,
	"diff":   true,
	"fetch":  true,
	"grep":   true,
	"log":    true,
	"push":   true,
	"stash":  true,
	"status": true,
}

func TestGitShimAliases_NamesMatchFrozenContract(t *testing.T) {
	var got []string
	for _, alias := range shellinit.GitShimAliases {
		got = append(got, alias.Name)
	}
	sort.Strings(got)

	want := append([]string(nil), frozenShimNames...)
	sort.Strings(want)

	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("shim alias names drifted from the frozen contract\n got: %v\nwant: %v", got, want)
	}
}

func TestGitShimAliases_TargetsResolve(t *testing.T) {
	customs := make(map[string]bool)
	for _, info := range gitcmd.SubCommandInfos() {
		customs[info.Name] = true
	}

	for _, alias := range shellinit.GitShimAliases {
		first := strings.Fields(alias.Target)[0]
		if !customs[first] && !passthroughFirstTokens[first] {
			t.Errorf("alias %q targets %q which is neither a gitcmd custom nor a known git subcommand", alias.Name, first)
		}
	}
}

func TestGitRegistryEntry_SubCommandsMatchGitcmd(t *testing.T) {
	entry, ok := commandIndex["git"]
	if !ok {
		t.Fatal("registry has no git command")
	}
	infos := gitcmd.SubCommandInfos()
	if len(entry.SubCommands) != len(infos) {
		t.Fatalf("registry git SubCommands len = %d, want %d", len(entry.SubCommands), len(infos))
	}
	for i, info := range infos {
		if entry.SubCommands[i].Name != info.Name {
			t.Fatalf("registry git SubCommands[%d] = %q, want %q", i, entry.SubCommands[i].Name, info.Name)
		}
	}
}
