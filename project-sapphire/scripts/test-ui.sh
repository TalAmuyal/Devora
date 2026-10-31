#!/usr/bin/env bash
# Runs the XCUITests (UITests/) against the app, or says why they can't run here.
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

results=build/ui-tests.xcresult
rm -rf "$results"
if ! xcodebuild -project App/DevoraSapphire.xcodeproj -scheme DevoraSapphire -configuration Release \
	-derivedDataPath build/DerivedData -destination 'platform=macOS,arch=arm64' -resultBundlePath "$results" test -quiet; then
	# `-quiet` names the failing tests but not why they failed; the result bundle has the messages.
	xcrun xcresulttool get test-results summary --path "$results" --compact |
		jq -r '.testFailures[] | "\(.testIdentifierString): \(.failureText)"' || true
	exit 1
fi
echo "The XCUITests passed"
