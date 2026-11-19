#!/usr/bin/env bash
# Checks the acceptance hang watchdog (hang-watchdog.ts) on real hangs, with a 3-second limit and no app: a step blocked
# in a synchronous spawn, and a run that doesn't exit after its last scenario, are named and stopped; a run that ends
# exits as it did without the watchdog.
# Usage: hang-watchdog.test.sh <project directory>, run with that project's Cucumber and tsx (Ember's and Sapphire's
# differ, and how tsx loads the watchdog's module is part of what's checked). The fixture is copied into the project's
# test-results/, where its steps import that project's Cucumber.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
cd "$1" || exit 1
fixtures=test-results/hang-watchdog-fixture
rm -rf "$fixtures"
mkdir -p "$(dirname "$fixtures")"
cp -R "$here/fixtures/hang-watchdog" "$fixtures"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch" "$fixtures"' EXIT
failures=0
fail() {
	printf 'FAIL: %s\n' "$1" >&2
	failures=$((failures + 1))
}

# Runs the fixture scenarios tagged $1, with the support files after it; its output is in $scratch/$1.out, the report in $scratch/$1.txt.
run() {
	local tag="$1" support=()
	shift
	for file in "$@"; do support+=(--import "$file"); done
	# The group's own stderr only takes bash's notice of a command a signal ended ("Terminated: 15"), which a stopped run is.
	{
		ACCEPTANCE_HANG_LIMIT_SECONDS=3 ACCEPTANCE_HANG_REPORT="$scratch/$tag.txt" \
			node --import tsx ./node_modules/.bin/cucumber-js "$fixtures/hangs.feature" --tags "@$tag" --import "$fixtures/hangs.steps.ts" ${support[@]+"${support[@]}"} \
			--format progress --format "../acceptance/hang-watchdog-formatter.ts:$scratch/$tag.progress" >"$scratch/$tag.out" 2>&1
	} 2>/dev/null
}

if run blocked; then
	fail "a blocked step: the run passed"
fi
grep -qF 'acceptance: no progress for 3 s since step "a step waits synchronously for a child process that doesn'\''t end" of scenario "A step blocked on a child process"' "$scratch/blocked.out" ||
	fail "a blocked step: the message doesn't name the step and its scenario: $(cat "$scratch/blocked.out")"
grep -qE '^ *[0-9]+ +[0-9]+ +[0-9:]+  sleep 117$' "$scratch/blocked.txt" ||
	fail "a blocked step: the report doesn't show the sleep under the run"
grep -qF 'node::SyncProcessRunner' "$scratch/blocked.txt" ||
	fail "a blocked step: the report's stack doesn't show the synchronous spawn"
grep -qxF 'the last line it printed' "$scratch/blocked.txt" ||
	fail "a blocked step: the report doesn't end with the output the harness reported"
if pgrep -qf '^sleep 117$'; then
	fail "a blocked step: the sleep it started outlived the run"
fi
grep -qF 'scenario "A step blocked on a child process"' "$scratch/blocked.progress" ||
	fail "a blocked step: the progress doesn't show the scenario that started"

if run lingering; then
	fail "a run that doesn't exit: it passed"
fi
grep -qF 'acceptance: no progress for 3 s since the last scenario finished, and the run should have exited' "$scratch/lingering.out" ||
	fail "a run that doesn't exit: the message doesn't say so: $(cat "$scratch/lingering.out")"

# A hang before any scenario: the same passing scenario, after a BeforeAll that blocks (its report goes to passing.txt).
if run passing "$fixtures/blocked-before-all.ts"; then
	fail "a blocked BeforeAll: the run passed"
fi
grep -qF 'acceptance: no progress for 3 s since the run started (its BeforeAll hooks run next, then the first scenario)' "$scratch/passing.out" ||
	fail "a blocked BeforeAll: the message doesn't say the run was before its first scenario: $(cat "$scratch/passing.out")"
rm -f "$scratch/passing.txt"

if ! run passing; then
	fail "a run that ends: it failed: $(cat "$scratch/passing.out")"
fi
if [ -e "$scratch/passing.txt" ]; then
	fail "a run that ends: it left a hang report"
fi

if [ "$failures" -gt 0 ]; then
	echo "hang-watchdog: $failures check(s) failed" >&2
	exit 1
fi
echo "hang-watchdog: all checks passed"
