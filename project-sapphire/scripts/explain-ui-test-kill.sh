#!/usr/bin/env bash
# Says why an XCUITest run failed when macOS killed its test runner to free disk space: with little space left, the
# cache_delete daemon purges app container caches and has RunningBoard terminate their apps, the runner among them,
# which XCTest reports only as "Test crashed with signal kill" (no crash report, nothing in the app's log).
# It reads the system log from the run's start, for RunningBoard's termination of our runner by cache_delete; it prints
# nothing when there's none. It only explains a failure: test-ui.sh fails the run either way.
# Usage: explain-ui-test-kill.sh <run start, "YYYY-MM-DD HH:MM:SS" local time>, or --log <file> (`log show` output).
set -euo pipefail

# PRODUCT_BUNDLE_IDENTIFIER of the UI test target (App/project.yml), with the suffix XCTest gives its runner.
runner=dev.devora.sapphire.uitests.xctrunner

if [ "$1" = --log ]; then
	log=$(cat "$2")
else
	# /usr/bin/log, not `log`: zsh has a builtin of that name.
	log=$(/usr/bin/log show --start "$1" --style compact \
		--predicate "process == \"runningboardd\" AND eventMessage CONTAINS \"CacheDeleteAppContainerCaches\" AND eventMessage CONTAINS \"$runner\"" 2>/dev/null) || exit 0
fi

kills=$(grep -F "CacheDeleteAppContainerCaches requesting termination" <<<"$log" | grep -F "$runner" | grep -F "Terminating" || true)
if [ -n "$kills" ]; then
	free=$(df -h "$HOME" | awk 'NR == 2 { print $4 }')
	echo "The UI test runner was killed by macOS freeing disk space (cache_delete purged its container), not by a test: $free free on the disk. Free disk space and run again."
	echo "$kills" | head -n 1
fi
