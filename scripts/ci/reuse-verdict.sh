#!/usr/bin/env bash
# Decides, for a push to master, which of CI's jobs a green pull request run would let it reuse: the same tree, tested
# green. It only reports; nothing reads its verdicts yet (step 1 of reusing PR results).
#
#   reuse-verdict.sh decide <pushed tree> <pr number, or none> <evidence file, or none>
#     Prints the verdicts as JSON, one per job of the evidence.
#   reuse-verdict.sh collect <repo> <pushed commit> <out dir>
#     Finds the commit's pull request and its latest completed CI run, and downloads that run's evidence to
#     <out dir>/reuse-evidence.json; prints the PR number, or "none" for a direct push. Needs GH_TOKEN with actions and
#     pull request read access.
set -euo pipefail

decide() {
	local tree=$1 pr=$2 evidence=$3
	if [ "$pr" = none ]; then
		jq -n '[{job: "*", verdict: "must run", reason: "a direct push, with no pull request"}]'
	elif [ "$evidence" = none ] || [ ! -s "$evidence" ]; then
		jq -n '[{job: "*", verdict: "must run", reason: "no evidence from the pull request (its last run was cancelled, or has none)"}]'
	else
		jq --arg tree "$tree" '
			.tree as $tested | .results | to_entries | map(
				{job: .key} + (
					if $tested != $tree then {verdict: "must run", reason: "a different tree (master moved after the pull request'"'"'s last run)"}
					elif .value != "success" then {verdict: "must run", reason: "not green on the pull request (\(.value))"}
					else {verdict: "would be reused", reason: "the same tree, green on the pull request"}
					end))' "$evidence"
	fi
}

collect() {
	local repo=$1 sha=$2 out=$3 pr head run
	pr=$(gh api "repos/$repo/commits/$sha/pulls" --jq '[.[] | select(.merged_at != null)] | first | .number // empty')
	if [ -z "$pr" ]; then
		echo none
		return
	fi
	head=$(gh api "repos/$repo/pulls/$pr" --jq .head.sha)
	run=$(gh api "repos/$repo/actions/workflows/ci.yml/runs?event=pull_request&head_sha=$head&status=completed" \
		--jq '.workflow_runs | sort_by(.created_at) | last | .id // empty')
	if [ -n "$run" ]; then
		gh run download "$run" --repo "$repo" --name reuse-evidence --dir "$out" >/dev/null 2>&1 || true
	fi
	echo "$pr"
}

case "${1:-}" in
	decide) shift; decide "$@" ;;
	collect) shift; collect "$@" ;;
	*) echo "usage: $0 decide <tree> <pr|none> <evidence|none> | collect <repo> <sha> <out dir>" >&2; exit 2 ;;
esac
