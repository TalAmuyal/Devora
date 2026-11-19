#!/usr/bin/env bash
# Runs the XCUITests (UITests/) against the app, or says why they can't run here.
# Arguments go to xcodebuild (stress-ui passes `-only-testing:` and `-test-iterations`).
set -euo pipefail
cd "$(dirname "$0")/.."

skip() {
	echo "Skipping the XCUITests: $1"
	exit 0
}

if [ "${SAPPHIRE_SKIP_UI_TESTS:-0}" = 1 ]; then
	skip "SAPPHIRE_SKIP_UI_TESTS=1"
fi
# XCUITest drives the real window server: a locked screen fails every test.
if ioreg -n Root -d1 | grep -q '"CGSSessionScreenIsLocked"=Yes'; then
	skip "the screen is locked"
fi

# The UI test runner is sandboxed, where git can't run (xcrun refuses to), so the git repo the tests copy into their fixtures is made here.
# xcodebuild passes TEST_RUNNER_-prefixed variables to the runner, without the prefix.
git_repo="$PWD/build/ui-test-fixtures/repo"
rm -rf "$git_repo"
git init -q -b main "$git_repo"
git -C "$git_repo" -c user.name=uitest -c user.email=uitest@localhost -c commit.gpgsign=false commit -q --allow-empty -m init
export TEST_RUNNER_SAPPHIRE_GIT_REPO="$git_repo"
# The Agent settings test chooses this template in the open panel. In the runner's container, Go to Folder would take the panel through all of ~/Library, whose rows make every snapshot of the panel slow.
agent_template="$PWD/build/ui-test-fixtures/agent-template/agent-template.md"
mkdir -p "$(dirname "$agent_template")"
printf 'You are {{agent.name}}.' >"$agent_template"
export TEST_RUNNER_SAPPHIRE_AGENT_TEMPLATE="$agent_template"
export TEST_RUNNER_DEVORA_TEST_CLAUDE_PROGRAM="$PWD/../project-core/crates/devora-agent/tests/support/fake_cli.py"
export TEST_RUNNER_DEVORA_FAKE_FIXTURES="$PWD/../project-core/crates/devora-agent/tests/fixtures/claude-2.1.312"
export TEST_RUNNER_SAPPHIRE_TEST_DEBI="$PWD/build/test-tools/debi"
# The sandboxed runner receives the shared, sanitized fixture through its environment.
TEST_RUNNER_DEVORA_HEALTH_FIXTURE_JSON="$(cat ../acceptance/fixtures/health-report.json)"
export TEST_RUNNER_DEVORA_HEALTH_FIXTURE_JSON

results=build/ui-tests.xcresult
rm -rf "$results"
passed=true
# Local time, the form `log show --start` takes.
started=$(date '+%Y-%m-%d %H:%M:%S')
# Each test gets at most 5 minutes (the slowest passing one takes about 1): one that hangs fails by name, with its screenshot (and a spindump, where the runner may take one) in the result bundle, and the run goes on.
if ! xcodebuild -project App/DevoraSapphire.xcodeproj -scheme DevoraSapphire -configuration Release \
	-derivedDataPath build/DerivedData -destination 'platform=macOS,arch=arm64' -resultBundlePath "$results" \
	-test-timeouts-enabled YES -default-test-execution-time-allowance 300 \
	-maximum-test-execution-time-allowance 300 "$@" test -quiet; then
	passed=false
	# `-quiet` names the failing tests but not why they failed; the result bundle has the messages.
	xcrun xcresulttool get test-results summary --path "$results" --compact |
		jq -r '.testFailures[] | "\(.testIdentifierString): \(.failureText)"' || true
	# A test that "crashed with signal kill" may have been macOS freeing disk space, not a test: say so when it was.
	./scripts/explain-ui-test-kill.sh "$started"
fi
# Whether they passed or not: a test that leaned on XCUITest's own scroll only flakes some of the time.
if [ -d "$results" ] && ! ./scripts/check-ui-scrolls.sh "$results"; then
	passed=false
fi
if [ "$passed" != true ]; then
	exit 1
fi
echo "The XCUITests passed"
