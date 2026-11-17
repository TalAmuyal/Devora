#!/usr/bin/env bash
# Reports each test's failures over the last CI runs, from the test-outcomes artifacts each job uploads
# (scripts/ci/test-outcomes/README.md): failures out of runs, on master and on pull requests apart, the last failure with
# its run, the median and slowest durations, and failures that passed when the job was re-run, the clearest flakes.
# Jobs GitHub cancelled before they started (no runner was acquired) are counted apart, as "not run": they aren't test failures.
# Usage: flake-report.sh [--runs N] [--test <regex>]   (mise run flake-report; uses gh's login)
set -euo pipefail
runs=30 filter=""
while [ $# -gt 0 ]; do
	case "$1" in
	--runs) runs="$2"; shift 2 ;;
	--test) filter="$2"; shift 2 ;;
	*) echo "usage: flake-report.sh [--runs N] [--test <regex>]" >&2; exit 2 ;;
	esac
done
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
# Artifacts never change once uploaded, so each run's are kept.
cache="${XDG_CACHE_HOME:-$HOME/.cache}/devora-flake-report"
mkdir -p "$cache"

listed=$(gh run list --repo "$repo" --workflow ci.yml --status completed --limit "$runs" --json databaseId,attempt,event,url,createdAt)
outcomes="$cache/report-outcomes.jsonl" not_run="$cache/report-not-run.jsonl"
: >"$outcomes"
: >"$not_run"
# Keyed by run and attempt: a run re-run since it was cached has a new attempt to fetch.
jq -r '.[] | "\(.databaseId) \(.attempt)"' <<<"$listed" | while read -r run attempts; do
	dir="$cache/$run-$attempts"
	if [ ! -e "$dir/.complete" ]; then
		rm -rf "$dir"
		mkdir -p "$dir"
		# A run from before outcomes were recorded has none: that's not an error.
		gh run download "$run" --repo "$repo" --pattern 'test-outcomes-*' --dir "$dir" >/dev/null 2>&1 || true
		# A counter, not seq: BSD seq counts down when the first number is the larger.
		attempt=1
		while [ "$attempt" -le "$attempts" ]; do
			gh api "repos/$repo/actions/runs/$run/attempts/$attempt/jobs" --paginate --jq '.jobs[]' >"$dir/jobs-$attempt.jsonl" 2>/dev/null || true
			attempt=$((attempt + 1))
		done
		touch "$dir/.complete"
	fi
	find "$dir" -name 'outcomes.jsonl' -exec cat {} + >>"$outcomes" 2>/dev/null || true
	# Cancelled before a runner took it: no runner name, no steps.
	cat "$dir"/jobs-*.jsonl 2>/dev/null |
		jq -c --arg run "$run" 'select(.conclusion == "cancelled" and (.runner_name // "") == "" and ((.steps // []) | length) == 0) | {run: $run, job: .name, attempt: .run_attempt, url: .html_url}' >>"$not_run" || true
done

jq -r -s --argjson listed "$listed" --arg filter "$filter" -f "$(dirname "$0")/flake-report.jq" "$outcomes"

if [ -s "$not_run" ]; then
	echo
	echo "Jobs not run, cancelled before a runner took them (capacity, not tests):"
	jq -r -s 'group_by(.job)[] | sort_by(.run | tonumber) | "  \(.[0].job): \(length) time(s), last \(last.url)"' "$not_run"
fi
