#!/usr/bin/env bash
# Prints what a pull request run tested, as JSON, for measuring how often master repeats a green PR check.
# The tree is the merge commit's that the run checked out (GitHub's `refs/pull/N/merge`), which is master's tree after a
# squash merge when master hasn't moved since; the results are the run's jobs' (`toJSON(needs)`).
# Usage: PR=<number> HEAD_SHA=<sha> NEEDS=<toJSON(needs)> reuse-evidence.sh
set -euo pipefail
jq -n \
	--arg tree "$(git rev-parse 'HEAD^{tree}')" \
	--argjson pr "$PR" \
	--arg head "$HEAD_SHA" \
	--argjson needs "$NEEDS" \
	'{tree: $tree, pr: $pr, head: $head, results: ($needs | map_values(.result))}'
