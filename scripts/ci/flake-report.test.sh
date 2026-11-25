#!/usr/bin/env bash
# Checks flake-report.jq on made-up outcomes: master and pull-request counts kept apart, a failure that passed on
# re-run flagged, skipped tests not counted as runs, tests that never failed left out, tests that needed their own retry
# listed apart whether they passed or not, the --test filter, and jobs not run told apart by why.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
failures=0
expect() {
	if [ "$3" != "$2" ]; then
		printf 'FAIL: %s\n--- expected:\n%s\n--- actual:\n%s\n' "$1" "$2" "$3" >&2
		failures=$((failures + 1))
	fi
}
listed='[
	{"databaseId": 1, "event": "push", "url": "https://ci/1", "createdAt": "2026-11-01T00:00:00Z"},
	{"databaseId": 2, "event": "push", "url": "https://ci/2", "createdAt": "2026-11-02T00:00:00Z"},
	{"databaseId": 3, "event": "pull_request", "url": "https://ci/3", "createdAt": "2026-11-03T00:00:00Z"},
	{"databaseId": 4, "event": "pull_request", "url": "https://ci/4", "createdAt": "2026-11-04T00:00:00Z"}]'
outcome() { printf '{"job":"%s","suite":"%s","name":"%s","outcome":"%s","duration":%s,"run_id":"%s","run_attempt":%s,"event":"%s"}\n' "$@"; }
retried() { printf '{"job":"%s","suite":"%s","name":"%s","outcome":"%s","duration":%s,"run_id":"%s","run_attempt":%s,"event":"%s","retries":%s}\n' "$@"; }
outcomes=$(
	outcome sapphire Health "real check" failed 12 1 1 push
	outcome sapphire Health "real check" passed 8 2 1 push
	outcome sapphire Health "real check" passed 9 3 1 pull_request
	outcome sapphire Settings "multiline save" failed 54 4 1 pull_request
	outcome sapphire Settings "multiline save" passed 50 4 2 pull_request
	outcome sapphire Settings "multiline save" passed 49 3 1 pull_request
	outcome test-ember-unit "app.test.ts" "always skipped" skipped null 1 1 push
	outcome test-ember-unit "app.test.ts" "always passes" passed 0.25 1 1 push
	retried sapphire KeyRouting "shift-shift" passed 20 1 1 push 0
	retried sapphire KeyRouting "shift-shift" passed 21 2 1 push 1
	retried sapphire KeyRouting "shift-shift" passed 22 3 1 pull_request 2
	retried sapphire KeyRouting "shift-shift" passed 20 4 1 pull_request 0
)
report() { jq -r -s --argjson listed "$listed" --arg filter "$1" -f "$here/flake-report.jq" <<<"$outcomes"; }

expect "the report" "$(printf '%s\n' \
	'Over the last 4 CI runs (2 on master, 2 on pull requests), 5 tests recorded:' \
	'' \
	'0/0 master, 1/2 PR  sapphire: Settings / multiline save' \
	'    failed, then passed on re-run, in 1 run(s)' \
	'    last failed: 2026-11-04T00:00:00Z https://ci/4' \
	'    duration: median 50s, slowest 54s' \
	'1/2 master, 0/1 PR  sapphire: Health / real check' \
	'    last failed: 2026-11-01T00:00:00Z https://ci/1' \
	'    duration: median 9s, slowest 12s' \
	'' \
	'Tests that needed their own retry (an XCUITest'"'"'s "… retry: …" attachment), passing or not:' \
	'1/2 master, 1/2 PR  sapphire: KeyRouting / shift-shift' \
	'    3 retries; last: 2026-11-03T00:00:00Z https://ci/3')" "$(report "")"

expect "the --test filter" "$(printf '%s\n' \
	'Over the last 4 CI runs (2 on master, 2 on pull requests), 2 tests recorded:' \
	'' \
	'No test failed.' \
	'' \
	'All matching tests:' \
	'  test-ember-unit: app.test.ts / always passes: failed 0/1 master, 0/0 PR; median 0.3s, slowest 0.3s' \
	'  test-ember-unit: app.test.ts / always skipped: failed 0/0 master, 0/0 PR; median -, slowest -')" "$(report "app.test")"

not_run() { printf '{"run":"%s","job":"%s","label":"%s","url":"https://ci/%s","reason":"%s"}\n' "$@"; }
expect "jobs not run, by why" "$(printf '%s\n' \
	'Jobs GitHub found no runner for (capacity, not tests):' \
	'  e2e-ember on macos-15: 1 time(s), last https://ci/1' \
	'  test-ember on macos-15: 2 time(s), last https://ci/3' \
	'Jobs a newer run of the same branch replaced before they started: 1' \
	'Jobs cancelled before they started, for another reason:' \
	'  sapphire on xcode-27: (no annotation) https://ci/4')" "$(
	{
		not_run 3 test-ember macos-15 3 'The job was not acquired by Runner of type hosted even after multiple attempts'
		not_run 1 test-ember macos-15 1 'The job was not acquired by Runner of type hosted even after multiple attempts'
		not_run 1 e2e-ember macos-15 1 'The job was not acquired by Runner of type hosted even after multiple attempts'
		not_run 2 test-ember macos-15 2 'Canceling since a higher priority waiting request for CI-refs/pull/1/merge exists'
		not_run 4 sapphire xcode-27 4 ''
	} | jq -r -s -f "$here/flake-report-not-run.jq"
)"

if [ "$failures" -gt 0 ]; then
	echo "$failures check(s) failed" >&2
	exit 1
fi
echo "flake-report: all checks passed"
