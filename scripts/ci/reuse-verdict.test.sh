#!/usr/bin/env bash
# Checks reuse-verdict.sh's decisions against known cases; a wrong "would be reused" would later skip a job that must run.
set -euo pipefail
here=$(dirname "$0")
fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT
failures=0

# The verdicts for one case, as "job: verdict (reason)" lines.
verdicts() {
	"$here/reuse-verdict.sh" decide "$@" | jq -r '.[] | "\(.job): \(.verdict) (\(.reason))"'
}

check() {
	local name=$1 expected=$2; shift 2
	local actual
	actual=$(verdicts "$@")
	if [ "$actual" != "$expected" ]; then
		printf 'FAIL: %s\nexpected:\n%s\nactual:\n%s\n' "$name" "$expected" "$actual" >&2
		failures=$((failures + 1))
	fi
}

cat >"$fixtures/evidence.json" <<'JSON'
{"tree": "aaa", "pr": 7, "head": "f00", "results": {"test": "success", "e2e-ember": "failure", "sapphire": "skipped", "test-ember": "cancelled"}}
JSON
: >"$fixtures/empty.json"

check "the same tree: green jobs are reused, the others run" \
	"test: would be reused (the same tree, green on the pull request)
e2e-ember: must run (not green on the pull request (failure))
sapphire: must run (not green on the pull request (skipped))
test-ember: must run (not green on the pull request (cancelled))" \
	aaa 7 "$fixtures/evidence.json"

check "a different tree: every job runs, green or not" \
	"test: must run (a different tree (master moved after the pull request's last run))
e2e-ember: must run (a different tree (master moved after the pull request's last run))
sapphire: must run (a different tree (master moved after the pull request's last run))
test-ember: must run (a different tree (master moved after the pull request's last run))" \
	bbb 7 "$fixtures/evidence.json"

check "a direct push" \
	"*: must run (a direct push, with no pull request)" \
	aaa none none

check "no evidence downloaded" \
	"*: must run (no evidence from the pull request (its last run was cancelled, or has none))" \
	aaa 7 none

check "an empty evidence file" \
	"*: must run (no evidence from the pull request (its last run was cancelled, or has none))" \
	aaa 7 "$fixtures/empty.json"

if [ "$failures" -gt 0 ]; then
	echo "$failures case(s) failed" >&2
	exit 1
fi
echo "reuse-verdict: all cases pass"
