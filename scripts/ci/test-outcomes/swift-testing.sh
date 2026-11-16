#!/usr/bin/env bash
# Prints one outcome per test function from a swift-testing event stream (`swift test --event-stream-output-path <file>
# --event-stream-version 0`): {"suite", "name", "outcome", "duration"}, the rest of the record coming from record.sh.
# A parameterized test is one outcome: it failed if any of its cases did. A known issue (withKnownIssue) isn't a failure.
# A function with no events (its suite was skipped) is skipped; one that started and never ended failed.
# Identity (README.md): the test's ID without its trailing `/<file>:<line>:<column>`, which moves when the file is edited;
# the suite is everything before the function (module, then nested suites), the name the function.
set -euo pipefail
jq -c -s '
	def stable: sub("/[^/]+\\.swift:[0-9]+:[0-9]+$"; "");
	([.[] | select(.kind == "test" and .payload.kind == "function") | .payload.id]) as $functions
	| [.[] | select(.kind == "event") | .payload] as $events
	| $functions[]
	| . as $id
	| [$events[] | select(.testID == $id)] as $own
	| (stable) as $test
	| ($test | if test("/") then capture("^(?<suite>.*)/(?<name>[^/]+)$") else capture("^(?<suite>[^.]*)\\.(?<name>.+)$") end) as $parts
	| ($own | map(select(.kind == "testStarted")) | first | .instant.absolute) as $start
	| ($own | map(select(.kind == "testEnded")) | last | .instant.absolute) as $end
	| {
		suite: $parts.suite,
		name: $parts.name,
		outcome: (
			if any($own[]; .kind == "testSkipped") or ($own | length) == 0 then "skipped"
			elif any($own[]; .kind == "issueRecorded" and (.issue.isKnown | not)) then "failed"
			elif $end == null then "failed" # started, never ended: it crashed the run
			else "passed" end),
		duration: (if $start != null and $end != null then ($end - $start) else null end)
	}
' "$1"
