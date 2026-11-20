#!/usr/bin/env bash
# Stops an XCUITest run at its first hang, naming the test, instead of letting every later test hang for its own allowance.
# Usage: ui-test-watchdog.sh <result bundle> <evidence dir> <parent pid> <process name>
# It watches the `<process name>` child of `<parent pid>` (test-ui.sh's xcodebuild), and reads the logs XCTest writes live
# under the bundle's Staging/<action>/Diagnostics (xcodebuild's `-quiet` output holds them for its end summary); a run in which
# it never finds that directory ends with a line saying it couldn't watch. It fires on the first:
# - test that exceeds its execution time allowance;
# - launch whose app never gets a process (XCTest's `isProcessIDValid` wait times out).
# Then it writes the evidence (what fired, the test, a screenshot, samples of the app and the runner, the Diagnostics logs)
# and stops that process: SIGINT, then SIGKILL past the grace. It signals only the process it found, by pid and start
# time, and it ends with that process, or with its parent.
# UI_TEST_WATCHDOG_INTERVAL (seconds between looks, default 5), UI_TEST_WATCHDOG_GRACE (seconds before SIGKILL, default
# 60) and UI_TEST_WATCHDOG_CAPTURE=0 (no screenshot or samples) are for its test.
set -uo pipefail
results=$1
evidence=$2
parent=$3
name=$4
interval=${UI_TEST_WATCHDOG_INTERVAL:-5}
grace=${UI_TEST_WATCHDOG_GRACE:-60}
capture=${UI_TEST_WATCHDOG_CAPTURE:-1}

started() { ps -o lstart= -p "$1" 2>/dev/null; }
# Whether the watched process is still the one found (a pid can be reused).
same() { [ -n "$2" ] && [ "$(started "$1")" = "$2" ]; }

# The process, once its parent has started it.
target=""
while [ -z "$target" ]; do
	kill -0 "$parent" 2>/dev/null || exit 0
	target=$(pgrep -P "$parent" -x "$name" | head -1)
	[ -n "$target" ] || sleep "$interval"
done
identity=$(started "$target")

# Every action's Diagnostics (`1_Test` today; another index or Xcode may name it differently).
diagnostics() { find "$results/Staging" -mindepth 2 -maxdepth 2 -type d -name Diagnostics 2>/dev/null; }
# The test a line names, as Class/test.
test_of() { sed -E -n "s/.*Test Case '-\\[[A-Za-z0-9_]+\\.([A-Za-z0-9_]+) ([A-Za-z0-9_]+)\\]'.*/\\1\\/\\2/p" | tail -1; }

reason=""
hung=""
watched=""
while same "$target" "$identity" && kill -0 "$parent" 2>/dev/null; do
	watched=$(diagnostics)
	if [ -n "$watched" ]; then
		# shellcheck disable=SC2086 # One path per line; no Staging path has a space in it.
		allowance=$(find $watched -name 'StandardOutputAndStandardError.txt' -exec grep -h "exceeded execution time allowance" {} + 2>/dev/null | head -1)
		if [ -n "$allowance" ]; then
			reason="exceeded its execution time allowance"
			hung=$(printf '%s\n' "$allowance" | test_of)
			break
		fi
		# shellcheck disable=SC2086
		if find $watched -name 'Session-*.log' -exec grep -l "reporting timed out failure with Expect value of 'isProcessIDValid'" {} + 2>/dev/null | grep -q .; then
			reason="launched the app, which never got a process"
			# shellcheck disable=SC2086
			hung=$(find $watched -name 'StandardOutputAndStandardError.txt' -exec grep -h "' started\\.$" {} + 2>/dev/null | test_of)
			break
		fi
	fi
	sleep "$interval"
done
if [ -z "$reason" ]; then
	if [ -z "$watched" ]; then
		echo "UI test watchdog: found no live XCTest logs (Staging/*/Diagnostics) in $results, so it couldn't watch this run for hangs"
	fi
	exit 0
fi

mkdir -p "$evidence"
message="UI test watchdog: ${hung:-a test} $reason; stopped the run, evidence in $evidence"
printf '%s\n' "$message" >"$evidence/reason.txt"
if [ "$capture" = 1 ]; then
	screencapture -x "$evidence/screen.png" 2>>"$evidence/capture.log" || echo "no screenshot" >>"$evidence/capture.log"
	# Only this build's app and runner: other agents' runs share the machine.
	built="$(cd "$results/.." && pwd)/DerivedData"
	for process in "Devora Sapphire" "DevoraSapphireUITests-Runner"; do
		for pid in $(pgrep -f "$built/.*/$process" 2>/dev/null); do
			sample "$pid" 3 -file "$evidence/sample-${process// /-}-$pid.txt" >/dev/null 2>>"$evidence/capture.log"
		done
	done
fi
for dir in $watched; do
	cp -R "$dir" "$evidence/Diagnostics-$(basename "$(dirname "$dir")")" 2>/dev/null
done

if same "$target" "$identity"; then
	kill -INT "$target" 2>/dev/null
	waited=0
	while same "$target" "$identity" && [ "$waited" -lt "$grace" ]; do
		sleep 1
		waited=$((waited + 1))
	done
	if same "$target" "$identity"; then
		kill -KILL "$target" 2>/dev/null
		echo "SIGKILL, after ${grace}s of SIGINT" >"$evidence/stopped-with.txt"
	else
		echo "SIGINT" >"$evidence/stopped-with.txt"
	fi
fi
