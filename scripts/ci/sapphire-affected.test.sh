#!/usr/bin/env bash
# Checks sapphire-affected.sh against known cases; a wrong "false" would skip the Sapphire job when it must run.
set -euo pipefail
here=$(dirname "$0")
failures=0
check() {
	local expected=$1; shift
	local actual
	actual=$(printf '%s\n' "$@" | "$here/sapphire-affected.sh")
	if [ "$actual" != "$expected" ]; then
		echo "FAIL: expected $expected for: $*" >&2
		failures=$((failures + 1))
	fi
}
check true project-sapphire/Sources/Store/Store.swift
check true project-core/crates/devora-state/src/store.rs
check true project-core/Cargo.lock
check true acceptance/features/workspace-hub.feature
check true project-ember/tests/support/fixture-helper.ts
check true mise.toml
check true .github/workflows/ci.yml
check true themes/default.json
check true VERSION
check true USER_GUIDE.md
check true scripts/ci/sapphire-affected.sh
check true some-new-dir/file.txt
check true project-ember/src/main.ts project-sapphire/App/project.yml
check true
check false project-ember/src/main.ts
check false project-ember/src-tauri/src/lib.rs project-ember/tests/steps/session.steps.ts
check false project-debi/cmd/main.go project-judgess/src/lib.rs
check false CHANGELOG.md docs/adrs/ADR-010-sapphire-ui-stack.md project-core/CLAUDE.md
check false bundler/macos-ember/bundle-fingerprint.sh .github/workflows/cd.yml
if [ "$failures" -gt 0 ]; then exit 1; fi
echo "sapphire-affected: all cases pass"
