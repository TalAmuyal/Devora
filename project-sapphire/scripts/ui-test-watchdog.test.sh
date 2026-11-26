#!/usr/bin/env bash
# Checks ui-test-watchdog.sh on Diagnostics trees with the lines of a real hung CI run (trimmed), with a `sleep` standing in
# for xcodebuild: what it fires on and names, what it leaves alone, and that it always ends with the run.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
watchdog="$here/ui-test-watchdog.sh"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
export UI_TEST_WATCHDOG_INTERVAL=0.2 UI_TEST_WATCHDOG_GRACE=2 UI_TEST_WATCHDOG_CAPTURE=0
failures=0
fail() {
	echo "FAIL: $1" >&2
	failures=$((failures + 1))
}

# A result bundle's Staging, as XCTest writes it during a run: the runner's output and its session log, under the action's
# directory (`1_Test` unless named).
bundle() {
	local results="$scratch/$1/ui-tests.xcresult"
	local dir="$results/Staging/${2:-1_Test}/Diagnostics/DevoraSapphireUITests-Iteration-1/DevoraSapphireUITests-1"
	mkdir -p "$dir"
	: >"$dir/StandardOutputAndStandardError.txt"
	: >"$dir/Session-DevoraSapphireUITests-2026-11-20_170040-ki5ban.log"
	echo "$results"
}
runner_output() { echo "$1/Staging/${2:-1_Test}/Diagnostics/DevoraSapphireUITests-Iteration-1/DevoraSapphireUITests-1/StandardOutputAndStandardError.txt"; }
session_log() { echo "$1/Staging/${2:-1_Test}/Diagnostics/DevoraSapphireUITests-Iteration-1/DevoraSapphireUITests-1/Session-DevoraSapphireUITests-2026-11-20_170040-ki5ban.log"; }

passed="Test Case '-[DevoraSapphireUITests.WorkspaceHubUITests testTheHubDrawsItsHeaderFilterAndHintsAsItOpens]' started.
Test Case '-[DevoraSapphireUITests.WorkspaceHubUITests testTheHubDrawsItsHeaderFilterAndHintsAsItOpens]' passed (6.563 seconds)."
hung_start="Test Case '-[DevoraSapphireUITests.WorkspaceHubUITests testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing]' started."
allowance="Test Case '-[DevoraSapphireUITests.WorkspaceHubUITests testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing]' exceeded execution time allowance of 5 minutes. The test may have hung; check Xcode's test report for additional diagnostics."
no_pid="17:11:48.312 DevoraSapphireUITests-Runner[67341:162934] <XCTWaiter: 0x777db57a80> reporting timed out failure with Expect value of 'isProcessIDValid' of dev.devora.sapphire.dev (pid:0) to be '1'"

# A stand-in for xcodebuild, named `sleep` as the watchdog is told: SIGINT ends it, as it ends a foreground xcodebuild
# (bash makes a background job ignore SIGINT, so it puts the default back first). Disowned, so that its end isn't reported.
stand_in() {
	perl -e '$SIG{INT} = "DEFAULT"; exec "sleep", @ARGV' "$1" &
	run=$!
	disown "$run"
}

# Waits for a process to end, a few seconds at most: the watchdog acts within its interval.
ended() {
	local pid=$1
	for _ in $(seq 1 50); do
		kill -0 "$pid" 2>/dev/null || return 0
		sleep 0.1
	done
	return 1
}

# 1. A test past its allowance: fires, names the test, keeps the logs, and stops the run.
results=$(bundle allowance)
stand_in 60
"$watchdog" "$results" "$scratch/allowance/hang" "$$" sleep &
dog=$!
printf '%s\n%s\n%s\n' "$passed" "$hung_start" "$allowance" >>"$(runner_output "$results")"
ended "$run" || fail "a test past its allowance didn't stop the run"
ended "$dog" || fail "the watchdog outlived the run it stopped"
expected="UI test watchdog: WorkspaceHubUITests/testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing exceeded its execution time allowance; stopped the run, evidence in $scratch/allowance/hang"
[ "$(cat "$scratch/allowance/hang/reason.txt" 2>/dev/null)" = "$expected" ] ||
	fail "the reason should name the test: $(cat "$scratch/allowance/hang/reason.txt" 2>/dev/null)"
[ "$(cat "$scratch/allowance/hang/stopped-with.txt" 2>/dev/null)" = SIGINT ] ||
	fail "a run that ends on SIGINT should be stopped with it: $(cat "$scratch/allowance/hang/stopped-with.txt" 2>/dev/null)"
[ -f "$scratch/allowance/hang/Diagnostics-1_Test/DevoraSapphireUITests-Iteration-1/DevoraSapphireUITests-1/StandardOutputAndStandardError.txt" ] ||
	fail "the evidence should keep the Diagnostics logs"

# 2. A launch with no process: fires at XCTest's timed-out pid wait, naming the test that started last.
results=$(bundle no-pid)
stand_in 60
"$watchdog" "$results" "$scratch/no-pid/hang" "$$" sleep &
dog=$!
printf '%s\n%s\n' "$passed" "$hung_start" >>"$(runner_output "$results")"
printf '%s\n' "$no_pid" >>"$(session_log "$results")"
ended "$run" || fail "a launch with no process didn't stop the run"
ended "$dog" || fail "the watchdog outlived the run it stopped (no process)"
grep -q "testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing launched the app, which never got a process" "$scratch/no-pid/hang/reason.txt" 2>/dev/null ||
	fail "the reason should name the test whose launch got no process: $(cat "$scratch/no-pid/hang/reason.txt" 2>/dev/null)"

# 1b. Another action's directory name: it still finds the logs.
results=$(bundle renamed 2_Test)
stand_in 60
"$watchdog" "$results" "$scratch/renamed/hang" "$$" sleep &
dog=$!
printf '%s\n%s\n' "$hung_start" "$allowance" >>"$(runner_output "$results" 2_Test)"
ended "$run" || fail "a hang logged under Staging/2_Test didn't stop the run"
ended "$dog" || fail "the watchdog outlived the run it stopped (2_Test)"
grep -q "testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing exceeded" "$scratch/renamed/hang/reason.txt" 2>/dev/null ||
	fail "a hang under Staging/2_Test should name the test"

# 3. Passing tests: it never fires, marks the run as watched, and ends with it.
results=$(bundle passing)
stand_in 1
"$watchdog" "$results" "$scratch/passing/hang" "$$" sleep >"$scratch/passing.out" &
dog=$!
printf '%s\n' "$passed" >>"$(runner_output "$results")"
ended "$run" || fail "the stand-in for a passing run didn't end by itself"
ended "$dog" || fail "the watchdog outlived a run that ended by itself"
[ ! -e "$scratch/passing/hang" ] || fail "passing tests shouldn't leave evidence"
[ ! -s "$scratch/passing.out" ] || fail "a watched run should end quietly: $(cat "$scratch/passing.out")"
[ -e "$scratch/passing/hang.watched" ] || fail "a run whose live logs it found should be marked as watched"

# 3b. No live logs at all (Xcode wrote them elsewhere): no mark, so test-ui.sh says the run wasn't watched.
mkdir -p "$scratch/unwatched/ui-tests.xcresult"
stand_in 1
"$watchdog" "$scratch/unwatched/ui-tests.xcresult" "$scratch/unwatched/hang" "$$" sleep &
dog=$!
ended "$run" || fail "the stand-in for an unwatched run didn't end by itself"
ended "$dog" || fail "the watchdog outlived an unwatched run"
[ ! -e "$scratch/unwatched/hang.watched" ] || fail "a run with no live logs shouldn't be marked as watched"

# 3c. Live logs found, then gone before the run ends (Staging folded into the finished bundle): still marked as watched.
results=$(bundle folded)
stand_in 60
"$watchdog" "$results" "$scratch/folded/hang" "$$" sleep &
dog=$!
for _ in $(seq 1 50); do
	[ -e "$scratch/folded/hang.watched" ] && break
	sleep 0.1
done
[ -e "$scratch/folded/hang.watched" ] || fail "a run whose live logs it found should be marked as watched (folded)"
rm -rf "$results/Staging"
kill -TERM "$run"
ended "$dog" || fail "the watchdog outlived a run whose Staging was folded"
[ -e "$scratch/folded/hang.watched" ] || fail "the mark should outlive the Staging it was made from"

# 4. Its parent gone (test-ui.sh killed): it ends without signalling anything.
results=$(bundle orphan)
(sleep 1) &
parent=$!
"$watchdog" "$results" "$scratch/orphan/hang" "$parent" sleep &
dog=$!
wait "$parent"
ended "$dog" || fail "the watchdog outlived its parent"

# 5. A run that ignores SIGINT: killed once the grace is over.
results=$(bundle stubborn)
(
	trap '' INT
	exec sleep 60
) &
run=$!
disown "$run"
"$watchdog" "$results" "$scratch/stubborn/hang" "$$" sleep &
dog=$!
printf '%s\n%s\n' "$hung_start" "$allowance" >>"$(runner_output "$results")"
ended "$dog" || fail "the watchdog didn't finish with a run that ignores SIGINT"
kill -0 "$run" 2>/dev/null && fail "a run that ignores SIGINT should be killed after the grace" && kill -KILL "$run"
[ "$(cat "$scratch/stubborn/hang/stopped-with.txt" 2>/dev/null)" = "SIGKILL, after 2s of SIGINT" ] ||
	fail "a run that ignores SIGINT should be killed: $(cat "$scratch/stubborn/hang/stopped-with.txt" 2>/dev/null)"

# 6–8. Whether the runner's WindowServer froze. Stand-ins on PATH: `sample` (the app's main thread waiting on SkyLight in
# every sample, or not), `screencapture` (answers, or hangs past the deadline) and `log` (GPU fence timeouts, or none),
# each noting its call; the app is `sleep`, run from this case's DerivedData as the build's app would be.
stubs="$scratch/stubs"
mkdir -p "$stubs"
cat >"$stubs/sample" <<'STUB'
#!/usr/bin/env bash
echo "sample $1" >>"$FAKE_CALLS"
count=3
[ -n "${FAKE_APP_WAITING:-}" ] && count=10
cat >"$4" <<SAMPLE
Call graph:
    10 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)
    + 10 start  (in dyld) + 6688  [0x1]
    +   $count SLSWindowServerClientCopySpaces  (in SkyLight) + 312  [0x2]
    +     $count mach_msg2_trap  (in libsystem_kernel.dylib) + 8  [0x3]
    10 Thread_2
SAMPLE
STUB
cat >"$stubs/screencapture" <<'STUB'
#!/usr/bin/env bash
echo "screencapture" >>"$FAKE_CALLS"
if [ -n "${FAKE_SHOT_HANGS:-}" ]; then
	echo $$ >"$FAKE_SHOT_PID"
	exec sleep 30
fi
echo png >"$2"
STUB
cat >"$stubs/log" <<'STUB'
#!/usr/bin/env bash
echo "log $*" >>"$FAKE_CALLS"
echo "Timestamp               Ty Process[PID:TID]"
[ -n "${FAKE_FENCES:-}" ] && echo "2026-11-25 15:40:19.606 E  WindowServer[163:3dd] [com.apple.coreanimation:Render] timed out fence 700079675426"
exit 0
STUB
chmod +x "$stubs"/*

# A hung test, its session log saying it started a minute ago, and the app running; returns once the watchdog has ended.
frozen() {
	local case=$1
	results=$(bundle "$case")
	local app="$scratch/$case/DerivedData/Build/Products/Release/Devora Sapphire.app/Contents/MacOS/Devora Sapphire"
	mkdir -p "$(dirname "$app")"
	ln -s "$(command -v sleep)" "$app"
	"$app" 60 &
	app_pid=$!
	disown "$app_pid"
	printf '%s DevoraSapphireUITests-Runner[1:2] -[XCTRunnerIDESession(_XCTestObservationInternal) testCaseWillStart:] DevoraSapphireUITests.WorkspaceHubUITests testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing\n' \
		"$(date -v-60S +%H:%M:%S.000)" >>"$(session_log "$results")"
	export FAKE_CALLS="$scratch/$case/calls" FAKE_SHOT_PID="$scratch/$case/shot.pid"
	stand_in 60
	PATH="$stubs:$PATH" UI_TEST_WATCHDOG_CAPTURE=1 UI_TEST_WATCHDOG_SCREENSHOT_DEADLINE=1 \
		"$watchdog" "$results" "$scratch/$case/hang" "$$" sleep &
	dog=$!
	printf '%s\n%s\n' "$hung_start" "$allowance" >>"$(runner_output "$results")"
	ended "$run" || fail "$case: the hang didn't stop the run"
	ended "$dog" || fail "$case: the watchdog outlived the run it stopped"
	kill "$app_pid" 2>/dev/null
}
verdict() { jq -r .verdict "$scratch/$1/hang/verdict.json" 2>/dev/null; }

# 6. Fences since the test started, and no answer (the screenshot hangs, the app waits on SkyLight): the runner's problem.
FAKE_FENCES=1 FAKE_SHOT_HANGS=1 FAKE_APP_WAITING=1 frozen runner
grep -q "^UI test watchdog: Runner problem, not the test: WindowServer stopped answering (GPU fence timeouts from 15:40:19; the screenshot got no answer within 1s, and the app's main thread was waiting on it in every sample) while WorkspaceHubUITests/testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing exceeded its execution time allowance" \
	"$scratch/runner/hang/reason.txt" 2>/dev/null || fail "a frozen WindowServer should be the runner's problem: $(cat "$scratch/runner/hang/reason.txt" 2>/dev/null)"
[ "$(verdict runner)" = runner ] || fail "the verdict should be the runner's: $(cat "$scratch/runner/hang/verdict.json" 2>/dev/null)"
[ "$(jq -r .test "$scratch/runner/hang/verdict.json" 2>/dev/null)" = "WorkspaceHubUITests/testTypingInTheFilterNarrowsTheListAndEscapeLeavesItBeforeClosing" ] ||
	fail "the verdict should name the test"
[ "$(sed -n 1p "$scratch/runner/calls" 2>/dev/null)" = "sample $app_pid" ] && grep -qx screencapture "$scratch/runner/calls" &&
	[ "$(grep -n -m1 -x screencapture "$scratch/runner/calls" | cut -d: -f1)" -gt 1 ] ||
	fail "the app should be sampled before the screenshot: $(cat "$scratch/runner/calls" 2>/dev/null)"
shot=$(cat "$scratch/runner/shot.pid" 2>/dev/null)
if [ -z "$shot" ] || kill -0 "$shot" 2>/dev/null; then fail "the screenshot past its deadline should be stopped"; fi
grep -q "no screenshot: WindowServer didn't answer within 1s" "$scratch/runner/hang/capture.log" 2>/dev/null ||
	fail "the capture log should say the screenshot got no answer"
since=$(sed -n 's/.*--last \([0-9]*\)s.*/\1/p' "$scratch/runner/calls")
[ -n "$since" ] && [ "$since" -ge 60 ] && [ "$since" -lt 120 ] ||
	fail "the fences should be looked for since the test started, a minute ago: --last ${since}s"
[ -s "$scratch/runner/hang/windowserver-fences.txt" ] || fail "the fence lines should be kept"

# 7. Fences, but WindowServer answered (the screenshot came, the app wasn't waiting): the test's, with the fact kept.
FAKE_FENCES=1 frozen answered
[ "$(verdict answered)" = test ] || fail "with WindowServer answering, the hang should be the test's: $(cat "$scratch/answered/hang/reason.txt" 2>/dev/null)"
grep -q "exceeded its execution time allowance; stopped the run, evidence in $scratch/answered/hang. WindowServer logged GPU fence timeouts from 15:40:19, but answered when the watchdog fired$" \
	"$scratch/answered/hang/reason.txt" 2>/dev/null || fail "the reason should keep the fences: $(cat "$scratch/answered/hang/reason.txt" 2>/dev/null)"

# 8. No answer, but no fences: the test's, with the fact kept.
FAKE_SHOT_HANGS=1 frozen unanswered
[ "$(verdict unanswered)" = test ] || fail "without fence timeouts, the hang should be the test's: $(cat "$scratch/unanswered/hang/reason.txt" 2>/dev/null)"
grep -q "WindowServer didn't answer (the screenshot got no answer within 1s), but logged no GPU fence timeouts$" \
	"$scratch/unanswered/hang/reason.txt" 2>/dev/null || fail "the reason should keep the unanswered screenshot: $(cat "$scratch/unanswered/hang/reason.txt" 2>/dev/null)"
[ ! -e "$scratch/unanswered/hang/windowserver-fences.txt" ] || fail "no fences, no fence file"

if [ "$failures" -gt 0 ]; then
	echo "ui-test-watchdog: $failures check(s) failed" >&2
	exit 1
fi
echo "ui-test-watchdog: all checks passed"
