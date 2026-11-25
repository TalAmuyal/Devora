#!/usr/bin/env bash
# Appends a CI job's test outcomes to one JSON-lines file, one object per test (README.md), from what its test runs wrote.
# Usage: record.sh <job> <out.jsonl> <format>=<path>... with format swift-testing, xcresult or cucumber.
# The run's identity comes from GitHub's environment (run id and attempt, commit, event, ref).
# It never fails the job: a result that's missing (its step didn't run) or unreadable is said in the log, and skipped.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
job="$1" out="$2"
shift 2
mkdir -p "$(dirname "$out")"
for input in "$@"; do
	format="${input%%=*}" path="${input#*=}"
	if [ ! -e "$path" ]; then
		echo "test-outcomes: no $format results at $path (its tests didn't run)"
		continue
	fi
	if ! outcomes=$("$here/$format.sh" "$path"); then
		echo "test-outcomes: couldn't read the $format results at $path"
		continue
	fi
	jq -c --arg job "$job" --arg format "$format" \
		--arg run_id "${GITHUB_RUN_ID:-}" --arg run_attempt "${GITHUB_RUN_ATTEMPT:-}" --arg sha "${GITHUB_SHA:-}" \
		--arg event "${GITHUB_EVENT_NAME:-}" --arg ref "${GITHUB_REF:-}" \
		'{job: $job, format: $format, suite, name, outcome, duration, run_id: $run_id, run_attempt: ($run_attempt | tonumber? // null), sha: $sha, event: $event, ref: $ref} + (if has("retries") then {retries} else {} end)' \
		<<<"$outcomes" >>"$out"
	echo "test-outcomes: $format at $path: $(jq -r .outcome <<<"$outcomes" | sort | uniq -c | awk '{printf "%s%s %s", sep, $1, $2; sep=", "}')"
done
exit 0
