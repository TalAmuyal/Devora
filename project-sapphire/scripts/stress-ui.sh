#!/usr/bin/env bash
# Runs XCUITests <times> times with every core busy and prints the tally (mise run stress-ui).
# Usage: stress-ui.sh <Class or Class/test> <times>
# testmanagerd launches the test runner and the app, not this process, so `taskpolicy -c background` can't reach them:
# busy loops, one per core, make the contention instead, for as long as the tests run. Every iteration counts: nothing retries.
set -euo pipefail
cd "$(dirname "$0")/.."
only="$1" times="$2"

burners=""
stop_burners() {
	for pid in $burners; do kill "$pid" 2>/dev/null || true; done
	burners=""
}
trap stop_burners EXIT
trap 'stop_burners; exit 130' INT TERM
for _ in $(seq "$(sysctl -n hw.ncpu)"); do
	yes >/dev/null &
	burners="$burners $!"
done

results=build/ui-tests.xcresult
# test-ui.sh skips (a locked screen) without a result bundle, so an old one mustn't be read as this run's.
rm -rf "$results"
status=0
if [ "$times" -gt 1 ]; then
	./scripts/test-ui.sh "-only-testing:DevoraSapphireUITests/$only" -test-iterations "$times" || status=$?
else
	# xcodebuild refuses -test-iterations 1.
	./scripts/test-ui.sh "-only-testing:DevoraSapphireUITests/$only" || status=$?
fi
stop_burners
if [ ! -d "$results" ]; then
	echo "stress-ui: the XCUITests didn't run (see above)"
	exit 1
fi
# Each test case has one Repetition per iteration; one iteration may have none, so its own result stands in.
tally=$(xcrun xcresulttool get test-results tests --path "$results" --compact | jq -r '
	.. | objects | select(.nodeType? == "Test Case")
	| . as $test
	| ([.children[]? | select(.nodeType == "Repetition")] | if length == 0 then [{name: "Repetition 1", result: $test.result}] else . end) as $runs
	| ($runs | map(select(.result != "Passed") | .name | sub("Repetition "; ""))) as $failed
	| "  \($test.name): \($runs | length - ($failed | length)) of \($runs | length) passed"
		+ (if ($failed | length) > 0 then " (failed iterations: \($failed | join(", ")); result bundle: '"$results"')" else "" end)')
if [ -z "$tally" ] && [ "$status" -ne 0 ]; then
	echo "stress-ui: the XCUITests failed before running any test (see above)"
	exit "$status"
fi
if [ -z "$tally" ]; then
	echo "stress-ui: no XCUITest matches \"$only\""
	exit 2
fi
echo "Under load (every core busy), per test:"
echo "$tally"
exit "$status"
