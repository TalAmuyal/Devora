#!/usr/bin/env bash
# Runs an acceptance command <times> times under load and prints the tally: Sapphire's stress-acceptance and Ember's stress-e2e.
# Usage: stress.sh <name> <times> <log-dir> -- <command...>
# The command runs on the efficiency cores only (taskpolicy -c background), as do the apps the harnesses spawn as its children,
# which reproduces a CI runner's contention. Every run counts: nothing retries. A run that matched no scenario is an error,
# since it would pass while testing nothing.
set -euo pipefail
name="$1" times="$2" logs="$3"
shift 3
[ "$1" = -- ] && shift
mkdir -p "$logs"
passed=0
failed=""
for run in $(seq "$times"); do
	log="$logs/run-$run.log"
	if taskpolicy -c background "$@" >"$log" 2>&1; then
		if grep -Eq '^(\[[a-z-]+\] )?0 scenarios' "$log"; then
			echo "stress: no scenario matches \"$name\" (see $log)" >&2
			exit 2
		fi
		passed=$((passed + 1))
		echo "run $run of $times: passed"
	else
		failed="$failed $run"
		echo "run $run of $times: failed"
	fi
done
echo "$passed of $times passed under load (\"$name\")"
if [ -n "$failed" ]; then
	echo "Failed runs:$failed; their logs are in $logs/run-<n>.log"
	exit 1
fi
