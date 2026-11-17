#!/usr/bin/env bash
# Checks with-ui-lock.sh on a lock file of its own: a second run waits, saying what holds the lock, and runs once it's free;
# a holder killed with -9 frees it at once; nested calls pass through, but a leaked or stale holder PID doesn't skip the lock;
# the command's exit status comes back. It waits on conditions (the lock file naming a holder), never on the clock.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
scratch=$(mktemp -d)
export DEVORA_UI_LOCK="$scratch/ui.lock"
unset DEVORA_UI_LOCK_HELD
pids=""
trap 'for pid in $pids; do kill -9 "$pid" 2>/dev/null; done; rm -rf "$scratch"' EXIT
failures=0
expect() {
	if [ "$3" != "$2" ]; then
		printf 'FAIL: %s\n  expected: %s\n  actual:   %s\n' "$1" "$2" "$3" >&2
		failures=$((failures + 1))
	fi
}
# Waits until the lock file names a holder: up to 500 looks, 10 ms apart.
holder() {
	local looks=0 pid
	while [ $looks -lt 500 ]; do
		pid=$(sed -n 's/^pid //p' "$DEVORA_UI_LOCK" 2>/dev/null)
		if [ -n "$pid" ] && [ "$pid" != "${1:-}" ]; then echo "$pid"; return; fi
		looks=$((looks + 1))
		sleep 0.01
	done
	echo "none"
}
# Waits until a file has a line: up to 500 looks, 10 ms apart.
appears() {
	local looks=0
	while [ $looks -lt 500 ]; do
		[ -s "$1" ] && return 0
		looks=$((looks + 1))
		sleep 0.01
	done
	return 1
}

# A holder, and a second run that must wait for it, then run once it's killed with -9.
"$here/with-ui-lock.sh" sleep 30 &
first=$!
pids="$pids $first"
expect "the first run holds the lock" "$first" "$(holder)"
"$here/with-ui-lock.sh" sh -c "echo ran > '$scratch/second'" 2>"$scratch/second.err" &
second=$!
pids="$pids $second"
appears "$scratch/second.err"
expect "the second run says what holds the lock" "with-ui-lock: waiting for the UI-test lock ($DEVORA_UI_LOCK), held by:" \
	"$(head -1 "$scratch/second.err")"
expect "it names the holder's command" "  command sleep 30" "$(grep '^  command' "$scratch/second.err")"
expect "the second run waits" "no" "$([ -e "$scratch/second" ] && echo yes || echo no)"
kill -9 "$first"
{ wait "$first"; } 2>/dev/null
wait "$second"
expect "killing the holder with -9 frees the lock for the waiting run" "ran" "$(cat "$scratch/second" 2>/dev/null)"
pkill -f "^sleep 30$" 2>/dev/null

# Nesting: a call under the holder passes through; a stale or foreign holder PID doesn't.
expect "a nested call passes through" "inner" \
	"$("$here/with-ui-lock.sh" sh -c "'$here/with-ui-lock.sh' echo inner")"
previous=$(sed -n 's/^pid //p' "$DEVORA_UI_LOCK")
"$here/with-ui-lock.sh" sleep 30 &
holding=$!
pids="$pids $holding"
expect "the next holder holds the lock" "$holding" "$(holder "$previous")"
DEVORA_UI_LOCK_HELD=$$ "$here/with-ui-lock.sh" sh -c "echo ran > '$scratch/leaked'" 2>/dev/null &
leaked=$!
pids="$pids $leaked"
DEVORA_UI_LOCK_HELD=999999 "$here/with-ui-lock.sh" sh -c "echo ran > '$scratch/stale'" 2>/dev/null &
stale=$!
pids="$pids $stale"
# Both must still be waiting for the holder: their commands haven't run while it holds the lock.
expect "a live PID that isn't the holder doesn't skip the lock" "no" "$([ -e "$scratch/leaked" ] && echo yes || echo no)"
expect "a stale PID doesn't skip the lock" "no" "$([ -e "$scratch/stale" ] && echo yes || echo no)"
kill -9 "$holding"
{ wait "$holding"; } 2>/dev/null
wait "$leaked" "$stale"
expect "they run once it's free" "ran ran" "$(cat "$scratch/leaked" "$scratch/stale" | tr '\n' ' ' | sed 's/ $//')"
pkill -f "^sleep 30$" 2>/dev/null

# The command's exit status, a signal's included.
"$here/with-ui-lock.sh" sh -c 'exit 7'
expect "the exit status comes back" "7" "$?"
"$here/with-ui-lock.sh" sh -c 'kill -TERM $$'
expect "a signal's death comes back as 128 + its number" "143" "$?"

if [ "$failures" -gt 0 ]; then
	echo "$failures check(s) failed" >&2
	exit 1
fi
echo "with-ui-lock: all checks passed"
