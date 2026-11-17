#!/usr/bin/env bash
# Prints one outcome per test from Vitest's JSON reporter (`--reporter=json --outputFile.json=<file>`): {"suite", "name",
# "outcome", "duration"}, the rest from record.sh. Skipped and todo tests are skipped.
# Identity (README.md): the test file's path from the repository's root, and the test's describe blocks and title.
# Usage: vitest.sh <report.json> [<root>], the root defaulting to the repository the script is in.
set -euo pipefail
root="${2:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}"
jq -c --arg root "${root%/}/" '
	.testResults[]
	| (.name | ltrimstr($root)) as $file
	| .assertionResults[]
	| {
		suite: $file,
		name: ((.ancestorTitles + [.title]) | join(" > ")),
		outcome: ({"passed": "passed", "failed": "failed"}[.status] // "skipped"),
		duration: (if .duration == null then null else .duration / 1000 end)
	}
' "$1"
