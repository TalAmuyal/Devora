#!/usr/bin/env bash
# Stops an XCUITest run at its first hang, naming the test, instead of letting every later test hang for its own allowance.
# Usage: ui-test-watchdog.sh <result bundle> <evidence dir> <parent pid> <process name>
# It watches the `<process name>` child of `<parent pid>` (test-ui.sh's xcodebuild), and reads the logs XCTest writes live
# under the bundle's Staging/<action>/Diagnostics (xcodebuild's `-quiet` output holds them for its end summary). The first time it
# finds them it creates `<evidence dir>.watched`, so that test-ui.sh can say when a run was never watched. It fires on the first:
# - test that exceeds its execution time allowance;
# - launch whose app never gets a process (XCTest's `isProcessIDValid` wait times out).
# Then it writes the evidence (what fired, the test, samples of the app and the runner, a screenshot, the Diagnostics logs,
# and whether the runner's WindowServer had frozen, with its verdict in verdict.json)
# and stops that process: SIGINT, then SIGKILL past the grace. It signals only the process it found, by pid and start
# time, and it ends with that process, or with its parent.
# UI_TEST_WATCHDOG_INTERVAL (seconds between looks, default 5), UI_TEST_WATCHDOG_GRACE (seconds before SIGKILL, default
# 60), UI_TEST_WATCHDOG_CAPTURE=0 (no screenshot, samples or log query), UI_TEST_WATCHDOG_SCREENSHOT_DEADLINE and
# UI_TEST_WATCHDOG_LOG_DEADLINE (seconds, default 10 and 20) are for its test.
set -uo pipefail
results=$1
evidence=$2
parent=$3
name=$4
interval=${UI_TEST_WATCHDOG_INTERVAL:-5}
grace=${UI_TEST_WATCHDOG_GRACE:-60}
capture=${UI_TEST_WATCHDOG_CAPTURE:-1}
screenshot_deadline=${UI_TEST_WATCHDOG_SCREENSHOT_DEADLINE:-10}
log_deadline=${UI_TEST_WATCHDOG_LOG_DEADLINE:-20}
screenshot_timed_out=""

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
# Runs a command as this script's own child for at most <seconds>, then stops that child, and only it: nothing between
# the watchdog firing and it stopping the run may wait unbounded. Fails when the command didn't finish in time.
bounded() {
	local limit=$1 waited=0 child
	shift
	"$@" &
	child=$!
	# Not reported as a job when stopped.
	disown "$child"
	while kill -0 "$child" 2>/dev/null && [ "$waited" -lt "$limit" ]; do
		sleep 1
		waited=$((waited + 1))
	done
	kill -0 "$child" 2>/dev/null || return 0
	kill -KILL "$child" 2>/dev/null
	return 1
}

reason=""
hung=""
watched=""
while same "$target" "$identity" && kill -0 "$parent" 2>/dev/null; do
	watched=$(diagnostics)
	if [ -n "$watched" ]; then
		[ -e "$evidence.watched" ] || touch "$evidence.watched"
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
[ -n "$reason" ] || exit 0

mkdir -p "$evidence"
if [ "$capture" = 1 ]; then
	# Samples first: `sample` doesn't need WindowServer, while a screenshot waits for it, and xcodebuild may end the hung
	# app and runner meanwhile. Only this build's app and runner: other agents' runs share the machine.
	built="$(cd "$results/.." && pwd)/DerivedData"
	for process in "Devora Sapphire" "DevoraSapphireUITests-Runner"; do
		for pid in $(pgrep -f "$built/.*/$process" 2>/dev/null); do
			sample "$pid" 3 -file "$evidence/sample-${process// /-}-$pid.txt" >/dev/null 2>>"$evidence/capture.log"
		done
	done
	# A screenshot waits for WindowServer.
	if ! bounded "$screenshot_deadline" screencapture -x "$evidence/screen.png" 2>>"$evidence/capture.log"; then
		screenshot_timed_out=1
		echo "no screenshot: WindowServer didn't answer within ${screenshot_deadline}s" >>"$evidence/capture.log"
	elif [ ! -s "$evidence/screen.png" ]; then
		echo "no screenshot" >>"$evidence/capture.log"
	fi
fi

# Whether the runner's WindowServer froze (CI's virtual GPU can stop completing work, freezing every app's drawing and
# accessibility): its GPU fence timeouts since the hung test started, and whether it answered when the watchdog fired (the
# screenshot, or the app's main thread found waiting in every sample on a WindowServer call, in SkyLight). Both together
# are the runner's problem, not the test's; either alone is kept for a reader to judge, and the hang stays the test's.
# The test's start, from its session log line (`… testCaseWillStart:] DevoraSapphireUITests.Class test`), on this clock.
since=""
searched="in the last 10 minutes (the test's start wasn't found)"
if [ -n "$hung" ]; then
	# shellcheck disable=SC2086
	began=$(find $watched -name 'Session-*.log' -exec grep -h "testCaseWillStart:\] [A-Za-z0-9_]*\.${hung%%/*} ${hung##*/}$" {} + 2>/dev/null |
		tail -1 | cut -d' ' -f1 | cut -d. -f1)
	if [ -n "$began" ] && start=$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date +%Y-%m-%d) $began" +%s 2>/dev/null); then
		since=$(($(date +%s) - start))
		searched="since the test started at $began"
		# A test that started before midnight.
		[ "$since" -ge 0 ] || since=$((since + 86400))
	fi
fi
first_fence=""
no_fence_data=""
app_waiting=""
if [ "$capture" = 1 ]; then
	# The log is history, but a loaded runner can take long to query it.
	if bounded "$log_deadline" log show --style compact --last "${since:-600}s" \
		--predicate 'process == "WindowServer" AND eventMessage CONTAINS "timed out fence"' \
		>"$evidence/windowserver-log.txt" 2>/dev/null; then
		grep "timed out fence" "$evidence/windowserver-log.txt" >"$evidence/windowserver-fences.txt"
		first_fence=$(head -1 "$evidence/windowserver-fences.txt" | awk '{print $2}' | cut -d. -f1)
		[ -s "$evidence/windowserver-fences.txt" ] || rm -f "$evidence/windowserver-fences.txt"
	else
		no_fence_data="no fence data: the log query didn't finish within ${log_deadline}s"
	fi
	rm -f "$evidence/windowserver-log.txt"
	for file in "$evidence"/sample-Devora-Sapphire-*.txt; do
		[ -f "$file" ] || continue
		# The main thread's sample count, then a SkyLight frame on its stack with that count: in every one of its samples.
		if awk '/Thread_[0-9]+ +DispatchQueue_1: com\.apple\.main-thread/ { total = $1; main = 1; next }
			main && /Thread_[0-9]+/ { main = 0 }
			main && $0 ~ ("^[ +!:|]*" total " ") && /\(in SkyLight\)/ { found = 1 }
			END { exit !found }' "$file"; then
			app_waiting=1
		fi
	done
fi
unanswered=""
[ -n "$screenshot_timed_out" ] && unanswered="the screenshot got no answer within ${screenshot_deadline}s"
[ -n "$app_waiting" ] && unanswered="${unanswered:+$unanswered, and }the app's main thread was waiting on it in every sample"
fences=""
[ -n "$first_fence" ] && fences="GPU fence timeouts from $first_fence, $searched"

if [ -n "$fences" ] && [ -n "$unanswered" ]; then
	verdict="runner"
	message="UI test watchdog: Runner problem, not the test: WindowServer stopped answering ($fences; $unanswered) while ${hung:-a test} $reason; stopped the run, evidence in $evidence"
else
	verdict="test"
	message="UI test watchdog: ${hung:-a test} $reason; stopped the run, evidence in $evidence"
	[ -n "$fences" ] && message="$message. WindowServer logged $fences, but answered when the watchdog fired"
	if [ -n "$unanswered" ]; then
		if [ -n "$no_fence_data" ]; then
			message="$message. WindowServer didn't answer ($unanswered), and there's $no_fence_data"
		else
			message="$message. WindowServer didn't answer ($unanswered), but logged no GPU fence timeouts $searched"
		fi
	elif [ -n "$no_fence_data" ]; then
		message="$message. There's $no_fence_data"
	fi
fi
printf '%s\n' "$message" >"$evidence/reason.txt"
# For the test outcomes (scripts/ci/test-outcomes/watchdog.sh), so that the flake report counts runner freezes apart.
jq -n --arg test "$hung" --arg verdict "$verdict" --arg reason "$reason" --arg fences "$first_fence" \
	--arg searched "${no_fence_data:-$searched}" --arg unanswered "$unanswered" \
	'{test: $test, verdict: $verdict, reason: $reason, fences_from: (if $fences == "" then null else $fences end), fences_searched: $searched, windowserver_unanswered: (if $unanswered == "" then null else $unanswered end)}' \
	>"$evidence/verdict.json"
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
