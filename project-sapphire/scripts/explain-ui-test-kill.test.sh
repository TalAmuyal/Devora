#!/usr/bin/env bash
# Checks explain-ui-test-kill.sh against the system log of a real kill (fixtures/ui-test-kill.txt): macOS's cache_delete
# had RunningBoard terminate our UI test runner to free disk space.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
explain="$here/explain-ui-test-kill.sh"
fixture="$here/fixtures/ui-test-kill.txt"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
failures=0
fail() {
	printf 'FAIL: %s\n' "$1" >&2
	failures=$((failures + 1))
}

out=$("$explain" --log "$fixture")
grep -qF "The UI test runner was killed by macOS freeing disk space" <<<"$out" ||
	fail "the runner's termination by cache_delete isn't explained: $out"
grep -qF "code:0xBADDD15C explanation:CacheDeleteAppContainerCaches" <<<"$out" ||
	fail "the explanation doesn't quote the termination: $out"

grep -vF "Terminating with context" "$fixture" >"$scratch/no-termination.log"
out=$("$explain" --log "$scratch/no-termination.log")
[ -z "$out" ] || fail "a termination request that wasn't carried out is explained: $out"

sed 's/dev\.devora\.sapphire\.uitests\.xctrunner/com.example.other/g' "$fixture" >"$scratch/another-app.log"
out=$("$explain" --log "$scratch/another-app.log")
[ -z "$out" ] || fail "another app's termination is explained as the runner's: $out"

if [ "$failures" -gt 0 ]; then
	echo "explain-ui-test-kill: $failures check(s) failed" >&2
	exit 1
fi
echo "explain-ui-test-kill: all checks passed"
