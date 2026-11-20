#!/usr/bin/env bash
# Prints, for the changed paths on stdin (one per line), whether each CI job group must run, as `<module>=true|false`
# lines (ADR-015): from scripts/ci/modules.lock.json, in the order scripts/ci/modules.toml describes. A path in
# `nothing` affects no module; one in `everything`, a build manifest, or one no module holds runs every module; any
# other path runs the modules whose inputs hold it. No paths at all runs every module, as nothing then shows what's safe.
# Usage: affected.sh [<lock>] < changed-paths
set -euo pipefail
lock=${1:-$(dirname "$0")/modules.lock.json}

jq -r -R -s --slurpfile lock "$lock" '
	$lock[0] as $map
	| def holds($paths): . as $p | any($paths[]; . as $d | if $d | endswith("/") then $p | startswith($d) else $p == $d end);
	def name: split("/") | last;
	[split("\n")[] | select(length > 0)] as $changed
	| ($map.modules | keys) as $modules
	| [$changed[]
		| if holds($map.nothing.paths) or (name | IN($map.nothing.names[])) then empty
		elif holds($map.everything.paths) or (name | IN($map.everything.manifests[])) then $modules[]
		else . as $p | [$modules[] | select(. as $m | $p | holds($map.modules[$m].inputs))]
			| if length == 0 then $modules[] else .[] end
		end
	] as $affected
	| (if ($changed | length) == 0 then $modules else $affected end) as $affected
	| $modules[] | "\(.)=\(. as $m | $affected | index($m) != null)"
'
