#!/usr/bin/env bash
# Prints the XCUITest the UI test watchdog stopped a run at, from its verdict.json (project-sapphire/scripts/ui-test-watchdog.sh):
# {"suite", "name", "outcome": "stopped", "duration": null, "watchdog"}, with `watchdog` its verdict: "runner" when the
# runner's WindowServer had frozen, "test" otherwise. The test's own result is still in the result bundle (xcresult.sh);
# this record is what lets the flake report count a runner freeze apart from the test's failures.
# Usage: watchdog.sh <verdict.json>
set -euo pipefail
jq -c 'select(.test != "") | (.test | capture("^(?<suite>[^/]+)/(?<name>.+)$")) as $id
	| {suite: $id.suite, name: ($id.name + "()"), outcome: "stopped", duration: null, watchdog: .verdict}' "$1"
