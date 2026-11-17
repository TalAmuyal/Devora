#!/usr/bin/env bash
# Checks check-ui-scrolls.sh against activities recorded from a real run (fixtures/ui-scroll-activities.json, whose Save click XCUITest scrolled to).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
check="$here/check-ui-scrolls.sh"
scrolled="$here/fixtures/ui-scroll-activities.json"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
failures=0
expect() {
	local what="$1" expected="$2" actual="$3"
	if [ "$actual" != "$expected" ]; then
		printf 'FAIL: %s\n  expected: %s\n  actual:   %s\n' "$what" "$expected" "$actual" >&2
		failures=$((failures + 1))
	fi
}
# Runs the check, printing its exit status and then the lines naming tests (or, on stderr, what it couldn't read).
run() {
	local out status
	out=$("$check" "$@" 2>"$scratch/stderr")
	status=$?
	echo "$status"
	grep '^  ' <<<"$out"
	grep '^check-ui-scrolls:' "$scratch/stderr" | sed "s|$scratch/||"
}

expect "a scroll fails the check, naming the test and the action" "$(printf '%s\n' 1 \
	'  AgentSettingsUITests/testMultilineSaveAndTemplateFilePicker(): Click "agent-save-role" Button')" \
	"$(run --json "$scrolled")"

jq 'del(.testRuns[0].activities[] | select(.title | startswith("Click")))' "$scrolled" >"$scratch/clean.json"
expect "no scroll passes" "0" "$(run --json "$scratch/clean.json")"

# With -test-iterations, each run is an iteration: only the one that scrolled is named.
jq --slurpfile clean "$scratch/clean.json" '.testRuns = [$clean[0].testRuns[0], .testRuns[0]]' "$scrolled" >"$scratch/iterations.json"
expect "iterations name the one that scrolled" "$(printf '%s\n' 1 \
	'  AgentSettingsUITests/testMultilineSaveAndTemplateFilePicker() (iteration 2): Click "agent-save-role" Button')" \
	"$(run --json "$scratch/clean.json" "$scratch/iterations.json")"

# The check never passes without having looked.
head -c 300 "$scrolled" >"$scratch/truncated.json"
expect "unreadable activities fail the check, naming them" "$(printf '%s\n' 1 \
	'check-ui-scrolls: couldn'\''t read the activities in truncated.json')" \
	"$(run --json "$scratch/clean.json" "$scratch/truncated.json")"
jq 'del(.testRuns)' "$scrolled" >"$scratch/unknown.json"
expect "activities in an unknown shape fail the check" "$(printf '%s\n' 1 \
	'check-ui-scrolls: couldn'\''t read the activities in unknown.json')" \
	"$(run --json "$scratch/unknown.json")"
expect "a bundle that can't be read fails the check" "$(printf '%s\n' 1 \
	'check-ui-scrolls: couldn'\''t read the tests of missing.xcresult')" \
	"$(run "$scratch/missing.xcresult")"

if [ "$failures" -gt 0 ]; then
	echo "check-ui-scrolls: $failures check(s) failed" >&2
	exit 1
fi
echo "check-ui-scrolls: all checks passed"
