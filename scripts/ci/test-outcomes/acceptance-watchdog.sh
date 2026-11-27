#!/usr/bin/env bash
# Prints the scenario the acceptance hang watchdog stopped a run at, from its hang.json (acceptance/hang-watchdog-worker.mjs):
# {"suite", "name", "outcome": "stopped", "duration": null, "watchdog"}, by cucumber.sh's identity (the feature's name and
# the scenario's), with `watchdog` its verdict: "runner" when the runner's WindowServer had frozen, "test" otherwise, by the
# UI test watchdog's rule. A stopped run writes no Cucumber results, so this is the stopped scenario's only record. A hang
# outside any scenario (a BeforeAll hook, a run that didn't exit) names none, and records nothing.
# Usage: acceptance-watchdog.sh <hang.json>
set -euo pipefail
jq -c 'select(.suite != null and .name != null) | {suite, name, outcome: "stopped", duration: null, watchdog: .verdict}' "$1"
