#!/usr/bin/env bash
# Fails when an XCUITest relied on XCUITest's own "Scroll element to visible", naming each test and the action it scrolled for (CLAUDE.md, "An XCUITest clicks only what's already in view").
# A bundle, or a test's activities, it can't read fails the check too: it never passes without having looked.
# Usage: check-ui-scrolls.sh <bundle.xcresult>, or check-ui-scrolls.sh --json <activities.json>... (`xcresulttool get test-results activities` outputs).
set -euo pipefail

if [ $# -eq 0 ] || { [ "$1" = --json ] && [ $# -lt 2 ]; }; then
	echo "usage: check-ui-scrolls.sh <bundle.xcresult>, or check-ui-scrolls.sh --json <activities.json>..." >&2
	exit 2
fi
if [ "$1" = --json ]; then
	shift
	activities=("$@")
else
	bundle="$1"
	scratch=$(mktemp -d)
	trap 'rm -rf "$scratch"' EXIT
	if ! tests=$(xcrun xcresulttool get test-results tests --path "$bundle" --compact) ||
		! ids=$(jq -r '.. | objects | select(.nodeType? == "Test Case") | .nodeIdentifier' <<<"$tests"); then
		echo "check-ui-scrolls: couldn't read the tests of $bundle" >&2
		exit 1
	fi
	activities=()
	n=0
	while IFS= read -r id; do
		[ -n "$id" ] || continue
		n=$((n + 1))
		if ! xcrun xcresulttool get test-results activities --path "$bundle" --test-id "$id" --compact >"$scratch/$n.json"; then
			echo "check-ui-scrolls: couldn't read the activities of $id in $bundle" >&2
			exit 1
		fi
		activities+=("$scratch/$n.json")
	done <<<"$ids"
	if [ "$n" -eq 0 ]; then
		exit 0
	fi
fi

found=""
for file in "${activities[@]}"; do
	# The action that scrolled is the nearest enclosing activity other than the event synthesis, e.g. `Click "agent-save-role" Button`.
	if ! jq -e '(.testIdentifier | type == "string") and (.testRuns | type == "array") and all(.testRuns[]; .activities | type == "array")' "$file" >/dev/null 2>&1 ||
		! lines=$(jq -r '
		def scrolls($chain):
			(.title // "") as $title
			| if $title == "Scroll element to visible" then $chain | map(select(. != "Synthesize event")) | last // "an action"
			else .childActivities[]? | scrolls($chain + [$title]) end;
		.testIdentifier as $test
		| (.testRuns | length) as $runs
		| .testRuns | to_entries[]
		| (if $runs > 1 then " (iteration \(.key + 1))" else "" end) as $iteration
		| .value.activities[] | scrolls([])
		| "  \($test)\($iteration): \(.)"
	' "$file" 2>/dev/null); then
		echo "check-ui-scrolls: couldn't read the activities in $file" >&2
		exit 1
	fi
	found+="$lines"$'\n'
done

found=$(sed '/^$/d' <<<"$found")
if [ -n "$found" ]; then
	echo "XCUITest scrolled a control into view itself before acting on it, a wheel scroll posted wherever the pointer rests (the race behind flaky clicks):"
	echo "$found"
	echo "Bring the control into view in the test first (e.g. scrollSettingsDetail(to:)); see project-sapphire/CLAUDE.md, \"An XCUITest clicks only what's already in view\"."
	exit 1
fi
