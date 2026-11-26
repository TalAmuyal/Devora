#!/usr/bin/env bash
# Prints one outcome per XCUITest from a result bundle: {"suite", "name", "outcome", "duration", "retries"}, the rest from record.sh.
# Identity (README.md): the class and the method, from the test's node identifier (`Class/testMethod()`).
# A test run with -test-iterations has one Repetition per iteration, and each is an outcome of its own.
# `retries` counts the test's own retries in that run: its attachments named `<Something> retry: …`
# (project-sapphire/UITests/CLAUDE.md). XCTest mangles an attachment's stored name, so they're counted by the activity
# that added each, which keeps the name ("Added attachment named '…'"). A test whose activities can't be read has no `retries`.
# Usage: xcresult.sh <bundle.xcresult>, or xcresult.sh --json <tests.json> [<activities.jsonl>] (the `xcresulttool get
# test-results tests` output, and the `xcresulttool get test-results activities` output of each test, one per line).
set -euo pipefail
if [ "$1" = --json ]; then
	tests=$(cat "$2")
	activities=$(cat "${3:-/dev/null}")
else
	tests=$(xcrun xcresulttool get test-results tests --path "$1" --compact)
	activities=$(jq -r '.. | objects | select(.nodeType? == "Test Case") | .nodeIdentifier' <<<"$tests" | while read -r id; do
		xcrun xcresulttool get test-results activities --path "$1" --test-id "$id" --compact || echo "test-outcomes: couldn't read the activities of $id" >&2
	done)
fi
# The activity XCTest records for each attachment, named by the convention.
retry='^Added attachment named .[^\x27]* retry: '
jq -c --arg retry "$retry" --slurpfile activities <(printf '%s' "$activities") '
	def outcome: {"Passed": "passed", "Failed": "failed", "Skipped": "skipped", "Expected Failure": "passed"}[.] // "failed";
	# A test whose only failure is the run being cancelled (the UI test watchdog stopping a run, which cancels the test
	# running then) never finished: skipped, not failed.
	def canceled: [.children[]? | select(.nodeType == "Failure Message") | .name] | length > 0 and all(. == "Testing was canceled");
	($activities | map({key: .testIdentifier, value: [.testRuns[] | [.. | objects | select(.title? | type == "string" and test($retry))] | length]}) | from_entries) as $retries
	| .. | objects | select(.nodeType? == "Test Case") | . as $case
	| (.nodeIdentifier | capture("^(?<suite>[^/]+)/(?<name>.+)$")) as $id
	| .nodeIdentifier as $test
	| ([.children[]? | select(.nodeType == "Repetition")] | if length == 0 then [null] else . end) | to_entries[] | .key as $run | .value as $repetition
	| {
		suite: $id.suite,
		name: $id.name,
		outcome: (($repetition // $case) | if .result == "Failed" and canceled then "skipped" else .result | outcome end),
		duration: (($repetition // $case).durationInSeconds // null)
	}
	+ (($retries[$test] // [])[$run] | if . == null then {} else {retries: .} end)
' <<<"$tests"
