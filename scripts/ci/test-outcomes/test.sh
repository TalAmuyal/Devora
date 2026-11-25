#!/usr/bin/env bash
# Checks the converters and record.sh against recorded results (fixtures/), including the identity rule (README.md):
# editing a test's file keeps its identity, renaming the test changes it.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
fixtures="$here/fixtures"
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
summary() { jq -r '"\(.suite) | \(.name) | \(.outcome)"' | sort; }

# swift-testing: every kind of outcome, nested suites and a free function.
expect "swift-testing outcomes" "$(printf '%s\n' \
	'ProbeTests | freeFunction() | passed' \
	'ProbeTests.Outer | fails() | failed' \
	'ProbeTests.Outer | knownIssue() | passed' \
	'ProbeTests.Outer | parameterized(value:) | failed' \
	'ProbeTests.Outer | passes() | passed' \
	'ProbeTests.Outer | skipped() | skipped' \
	'ProbeTests.Outer/Inner | nested() | passed')" \
	"$("$here/swift-testing.sh" "$fixtures/swift-testing-events.jsonl" | summary)"
expect "swift-testing durations come from the start and end instants" "true" \
	"$("$here/swift-testing.sh" "$fixtures/swift-testing-events.jsonl" | jq -s 'map(select(.outcome != "skipped") | .duration > 0) | all')"
# An edit above a test moves its line: the identity stays.
sed -E 's/ProbeTests\.swift:([0-9]+):/ProbeTests.swift:1\1:/g' "$fixtures/swift-testing-events.jsonl" >"$scratch/edited.jsonl"
expect "swift-testing identity survives an edit" \
	"$("$here/swift-testing.sh" "$fixtures/swift-testing-events.jsonl" | summary)" \
	"$("$here/swift-testing.sh" "$scratch/edited.jsonl" | summary)"
sed 's/passes()/stillPasses()/g' "$fixtures/swift-testing-events.jsonl" >"$scratch/renamed.jsonl"
expect "swift-testing identity follows a rename" "ProbeTests.Outer | stillPasses() | passed" \
	"$("$here/swift-testing.sh" "$scratch/renamed.jsonl" | summary | grep -F 'Passes()')"

# XCUITests: failed, passed and skipped tests, and one outcome per iteration.
expect "xcresult outcomes" "$(printf '%s\n' \
	'GlobalPauseUITests | testToolbarHoldSurvivesRelaunchAndThePaletteResumesIt() | failed' \
	'PreviewUITests | testDraggingTheDividerResizesThePane() | passed' \
	'PreviewUITests | testScreenshots() | skipped')" \
	"$("$here/xcresult.sh" --json "$fixtures/xcresult-outcomes.json" | summary)"
expect "xcresult iterations" "10 ErrorBannerUITests | testClickingTheCrossDismissesTheBanner() | passed" \
	"$("$here/xcresult.sh" --json "$fixtures/xcresult-iterations.json" | summary | uniq -c | sed 's/^ *//')"
expect "xcresult retries, per iteration, by the attachment name's convention" "$(printf '%s\n' \
	'ScratchRetryShapeTests | testAttachesARetry() | 1' \
	'ScratchRetryShapeTests | testAttachesARetry() | 0')" \
	"$("$here/xcresult.sh" --json "$fixtures/xcresult-retries-tests.json" "$fixtures/xcresult-retries-activities.jsonl" | jq -r '"\(.suite) | \(.name) | \(.retries)"')"
expect "xcresult without activities has no retries" "false" \
	"$("$here/xcresult.sh" --json "$fixtures/xcresult-iterations.json" | jq -s 'any(has("retries"))')"

# cucumber: a scenario, an outline's rows, and failed and skipped scenarios.
expect "cucumber scenario" "File preview pane | Refreshing an existing path preserves its stacked siblings | passed" \
	"$("$here/cucumber.sh" "$fixtures/cucumber.json" | summary)"
expect "cucumber outline rows" "$(printf '%s\n' \
	'Agent chat panel | Permission shortcuts deny or always allow #1 | passed' \
	'Agent chat panel | Permission shortcuts deny or always allow #2 | passed')" \
	"$("$here/cucumber.sh" "$fixtures/cucumber-outline.json" | summary)"
jq '.[0].elements[0].steps[2].result.status = "failed" | .[0].elements[1].steps |= map(.result.status = "skipped")' \
	"$fixtures/cucumber-outline.json" >"$scratch/statuses.json"
expect "cucumber failed and skipped" "$(printf '%s\n' \
	'Agent chat panel | Permission shortcuts deny or always allow #1 | failed' \
	'Agent chat panel | Permission shortcuts deny or always allow #2 | skipped')" \
	"$("$here/cucumber.sh" "$scratch/statuses.json" | summary)"
jq '.[0].uri = "../acceptance/features/moved.feature" | .[0].elements[].line += 40' "$fixtures/cucumber.json" >"$scratch/edited.json"
expect "cucumber identity survives an edit" \
	"$("$here/cucumber.sh" "$fixtures/cucumber.json" | summary)" \
	"$("$here/cucumber.sh" "$scratch/edited.json" | summary)"
jq '.[0].elements[0].name = "Refreshing a path keeps its siblings"' "$fixtures/cucumber.json" >"$scratch/renamed.json"
expect "cucumber identity follows a rename" "File preview pane | Refreshing a path keeps its siblings | passed" \
	"$("$here/cucumber.sh" "$scratch/renamed.json" | summary)"

# Rust: unit, integration and doc-tests by package; failed, ignored (with or without a reason) and should-panic tests.
expect "rust outcomes" "$(printf '%s\n' \
	'debi | repo::tests::names_sort | passed' \
	'debi | repo::tests::rejects_escape | passed' \
	'debi/repo | usage_errors_and_empty_lists_have_the_documented_exit_codes | passed' \
	'devora_agent (doc) | crates/devora-agent/src/lib.rs - version::check | passed' \
	'devora_agent | session::tests::pinned_cli_only | skipped' \
	'devora_agent | session::tests::pinned_cli_reason | skipped' \
	'devora_agent | session::tests::stop_grace | failed' \
	'devora_agent/repo | add_refuses_a_mismatch | passed')" \
	"$("$here/rust.sh" "$fixtures/rust-output.txt" | summary)"
# A rebuild changes the binaries' hashes, and an edit moves a doc-test's line: the identities stay.
sed -E 's/-[0-9a-f]{16}\)/-0123456789abcdef)/; s/\(line 12\)/(line 40)/' "$fixtures/rust-output.txt" >"$scratch/edited.log"
expect "rust identity survives a rebuild and an edit" \
	"$("$here/rust.sh" "$fixtures/rust-output.txt" | summary)" \
	"$("$here/rust.sh" "$scratch/edited.log" | summary)"

# Vitest: the file from the repository's root, describe blocks and title; failed and skipped tests.
expect "vitest outcomes" "$(printf '%s\n' \
	'project-ember/scripts/__tests__/app-bundle.test.ts | findAppBundle > returns null when the bundle directory does not exist | passed' \
	'project-ember/scripts/__tests__/app-bundle.test.ts | findAppBundle > returns null when the directory contains no .app | failed' \
	'project-ember/scripts/__tests__/bundle-fingerprint.test.ts | bundle-fingerprint.sh with explicit inputs > is independent of file creation order | skipped' \
	'project-ember/scripts/__tests__/bundle-fingerprint.test.ts | bundle-fingerprint.sh with explicit inputs > prints a sha256 hex digest and is deterministic across calls | passed')" \
	"$("$here/vitest.sh" "$fixtures/vitest.json" /home/runner/work/Devora/Devora | summary)"
sed 's|/home/runner/work/Devora/Devora/|/Users/someone/Devora/|' "$fixtures/vitest.json" >"$scratch/moved.json"
expect "vitest identity is the same in another checkout" \
	"$("$here/vitest.sh" "$fixtures/vitest.json" /home/runner/work/Devora/Devora | summary)" \
	"$("$here/vitest.sh" "$scratch/moved.json" /Users/someone/Devora | summary)"

# record.sh: the run's identity on every outcome, and a missing or unreadable result logged, never failing.
echo 'not json' >"$scratch/broken.json"
log=$(GITHUB_RUN_ID=42 GITHUB_RUN_ATTEMPT=2 GITHUB_SHA=abc GITHUB_EVENT_NAME=pull_request GITHUB_REF=refs/pull/7/merge \
	"$here/record.sh" sapphire "$scratch/out/outcomes.jsonl" \
	"cucumber=$fixtures/cucumber.json" "cucumber=$scratch/missing.json" "cucumber=$scratch/broken.json" 2>&1)
expect "record.sh exits 0" "0" "$?"
expect "record.sh adds the run" '{"job":"sapphire","format":"cucumber","suite":"File preview pane","name":"Refreshing an existing path preserves its stacked siblings","outcome":"passed","run_id":"42","run_attempt":2,"sha":"abc","event":"pull_request","ref":"refs/pull/7/merge"}' \
	"$(jq -c 'del(.duration)' "$scratch/out/outcomes.jsonl")"
expect "record.sh says what it recorded and what it couldn't" "$(printf '%s\n' \
	"test-outcomes: cucumber at $fixtures/cucumber.json: 1 passed" \
	"test-outcomes: no cucumber results at $scratch/missing.json (its tests didn't run)" \
	"test-outcomes: couldn't read the cucumber results at $scratch/broken.json")" \
	"$(grep '^test-outcomes' <<<"$log")"

if [ "$failures" -gt 0 ]; then
	echo "$failures check(s) failed" >&2
	exit 1
fi
echo "test-outcomes: all checks passed"
