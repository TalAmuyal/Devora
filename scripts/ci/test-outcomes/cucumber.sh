#!/usr/bin/env bash
# Prints one outcome per scenario from cucumber-js's JSON formatter (`--format json:<file>`): {"suite", "name", "outcome",
# "duration"}, the rest from record.sh. The duration includes the scenario's hooks.
# A scenario failed if a step or hook failed or is undefined, ambiguous or pending (cucumber-js fails the run for each);
# it's skipped if, otherwise, a step was skipped.
# Identity (README.md): the feature's name and the scenario's, never `uri:line`, which moves when the file is edited.
# A Scenario Outline's rows share their name, so each gets its position among them (" #2"); reordering the rows renames them.
set -euo pipefail
jq -c '
	.[] | .name as $feature
	| [.elements[] | select(.type == "scenario")]
	| to_entries as $scenarios
	| $scenarios[]
	| .key as $index
	| .value as $scenario
	| ([$scenarios[] | select(.key <= $index and .value.keyword == "Scenario Outline" and .value.name == $scenario.name)] | length) as $row
	| [$scenario.steps[]?, $scenario.before[]?, $scenario.after[]? | .result.status] as $statuses
	| {
		suite: $feature,
		name: ($scenario.name + (if $scenario.keyword == "Scenario Outline" then " #\($row)" else "" end)),
		outcome: (
			if any($statuses[]; . == "failed" or . == "undefined" or . == "ambiguous" or . == "pending") then "failed"
			elif any($statuses[]; . == "skipped") then "skipped"
			else "passed" end),
		duration: ([$scenario.steps[]?, $scenario.before[]?, $scenario.after[]? | .result.duration // 0] | add / 1e9)
	}
' "$1"
