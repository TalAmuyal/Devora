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

xcodebuild -project App/DevoraSapphire.xcodeproj -scheme DevoraSapphire -configuration Release \
	-derivedDataPath build/DerivedData -destination 'platform=macOS,arch=arm64' test -quiet
echo "The XCUITests passed"
