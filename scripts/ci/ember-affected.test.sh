#!/usr/bin/env bash
# Checks ember-affected.sh against known cases; a wrong "false" would skip Ember's jobs when they must run.
set -euo pipefail
here=$(dirname "$0")
root=$(cd "$here/../.." && pwd)
failures=0
check() {
	local expected=$1; shift
	local actual
	actual=$(printf '%s\n' "$@" | "$here/ember-affected.sh")
	if [ "$actual" != "$expected" ]; then
		echo "FAIL: expected $expected for: $*" >&2
		failures=$((failures + 1))
	fi
}
check true project-ember/src/main.ts
check true project-ember/src-tauri/src/lib.rs
check true project-core/crates/devora-state/src/store.rs
check true project-core/crates/devora-ffi/src/lib.rs
check true project-core/Cargo.lock
check true web-renderer/src/index.ts
check true acceptance/features/workspace-hub.feature
check true bundler/macos-ember/bundle-ember.sh
check true project-debi/go.mod
check true project-judgess/src/lib.rs
check true project-status-line/main.go
check true project-code-cleanup/cc-plugin/plugin.json
check true themes/catppuccin-macchiato.conf
check true USER_GUIDE.md
check true base-icon-ember.png
check true mise.toml
check true .github/workflows/ci.yml
# A Claude Code bump (scripts/ci/tools/README.md) must run both apps' CI: that's what it's for.
check true scripts/ci/tools/package.json scripts/ci/tools/package-lock.json
check true scripts/ci/ember-affected.sh
check true project-core/CLAUDE.md
check true some-new-dir/file.txt
check true project-sapphire/Sources/Store/Store.swift project-ember/src/main.ts
check true
check false project-sapphire/Sources/Store/Store.swift
check false project-sapphire/App/project.yml CHANGELOG.md
check false docs/adrs/ADR-014-native-agent-transcript.md CLAUDE.md AGENTS.md README.md
check false base-icon-sapphire.png
check false .github/workflows/cd.yml

# What Ember bundles must always count: every tracked file under a bundled source runs Ember's jobs, so a file that
# starts being bundled can't skip them silently. CHANGELOG.md is the one exception (see ember-affected.sh).
while IFS= read -r source; do
	[ "$source" = CHANGELOG.md ] && continue
	while IFS= read -r file; do
		[ "$(printf '%s\n' "$file" | "$here/ember-affected.sh")" = true ] && continue
		echo "FAIL: $file is bundled into Ember (under $source) but counts as not affecting it" >&2
		failures=$((failures + 1))
	done < <(git -C "$root" ls-files -- "$source")
done < <("$root/bundler/macos-ember/populate-app-resources.sh" --app ember --list-sources)

if [ "$failures" -gt 0 ]; then exit 1; fi
echo "ember-affected: all cases pass"
