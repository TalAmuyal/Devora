#!/usr/bin/env bash
# Prints one outcome per XCUITest from a result bundle: {"suite", "name", "outcome", "duration"}, the rest from record.sh.
# Identity (README.md): the class and the method, from the test's node identifier (`Class/testMethod()`).
# A test run with -test-iterations has one Repetition per iteration, and each is an outcome of its own.
# Usage: xcresult.sh <bundle.xcresult>, or xcresult.sh --json <tests.json> (the `xcresulttool get test-results tests` output).
set -euo pipefail
if [ "$1" = --json ]; then
	tests=$(cat "$2")
else
	tests=$(xcrun xcresulttool get test-results tests --path "$1" --compact)
fi
jq -c '
	def outcome: {"Passed": "passed", "Failed": "failed", "Skipped": "skipped", "Expected Failure": "passed"}[.] // "failed";
	.. | objects | select(.nodeType? == "Test Case")
	| (.nodeIdentifier | capture("^(?<suite>[^/]+)/(?<name>.+)$")) as $id
	| ([.children[]? | select(.nodeType == "Repetition")] | if length == 0 then [null] else . end)[] as $repetition
	| {
		suite: $id.suite,
		name: $id.name,
		outcome: (($repetition // .).result | outcome),
		duration: (($repetition // .).durationInSeconds // null)
	}
' <<<"$tests"
