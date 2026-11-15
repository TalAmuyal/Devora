#!/usr/bin/env bash
# Decides, for a push to master, which of CI's jobs a green pull request run would let it reuse: the same tree, tested
# green. It only reports; nothing reads its verdicts yet (step 1 of reusing PR results).
#
#   reuse-verdict.sh decide <pushed tree> <pr number, or none> <evidence status> [<evidence file>]
#     Prints the verdicts as JSON, one per job of the evidence. The status is collect's: found (with the file), no-run,
#     no-evidence or download-failed.
#   reuse-verdict.sh collect <repo> <pushed commit> <out dir>
#     Finds the commit's pull request and its latest completed CI run, and downloads that run's evidence to
#     <out dir>/reuse-evidence.json; prints the PR number (or "none" for a direct push) and the evidence status. A failed
#     download is its own status, so trouble reaching the evidence never passes for a run that had none. Needs GH_TOKEN
#     with actions and pull request read access.
set -euo pipefail

decide() {
	local tree=$1 pr=$2 status=$3 evidence=${4:-}
	local reason
	case "$pr:$status" in
		none:*) reason="a direct push, with no pull request" ;;
		*:no-run) reason="no completed CI run for the pull request's last commit" ;;
		*:no-evidence) reason="no evidence from the pull request's last run (it predates the evidence job, or was cancelled)" ;;
		*:download-failed) reason="the pull request's evidence couldn't be downloaded" ;;
		*:found) reason= ;;
		*) echo "unknown evidence status: $status" >&2; return 2 ;;
	esac
	if [ -n "$reason" ]; then
		jq -n --arg reason "$reason" '[{job: "*", verdict: "must run", reason: $reason}]'
		return
	fi
	jq --arg tree "$tree" '
		.tree as $tested | .results | to_entries | map(
			{job: .key} + (
				if $tested != $tree then {verdict: "must run", reason: "a different tree (master moved after the pull request'"'"'s last run)"}
				elif .value != "success" then {verdict: "must run", reason: "not green on the pull request (\(.value))"}
				else {verdict: "would be reused", reason: "the same tree, green on the pull request"}
				end))' "$evidence"
}

collect() {
	local repo=$1 sha=$2 out=$3 pr head run artifacts
	pr=$(gh api "repos/$repo/commits/$sha/pulls" --jq '[.[] | select(.merged_at != null)] | first | .number // empty')
	if [ -z "$pr" ]; then
		echo "none none"
		return
	fi
	head=$(gh api "repos/$repo/pulls/$pr" --jq .head.sha)
	run=$(gh api "repos/$repo/actions/workflows/ci.yml/runs?event=pull_request&head_sha=$head&status=completed" \
		--jq '.workflow_runs | sort_by(.created_at) | last | .id // empty')
	if [ -z "$run" ]; then
		echo "$pr no-run"
		return
	fi
	artifacts=$(gh api "repos/$repo/actions/runs/$run/artifacts?name=reuse-evidence" --jq .total_count)
	if [ "$artifacts" = 0 ]; then
		echo "$pr no-evidence"
	elif gh run download "$run" --repo "$repo" --name reuse-evidence --dir "$out" >&2 && [ -s "$out/reuse-evidence.json" ]; then
		echo "$pr found"
	else
		echo "$pr download-failed"
	fi
}

case "${1:-}" in
	decide) shift; decide "$@" ;;
	collect) shift; collect "$@" ;;
	*) echo "usage: $0 decide <tree> <pr|none> <status> [<evidence>] | collect <repo> <sha> <out dir>" >&2; exit 2 ;;
esac
