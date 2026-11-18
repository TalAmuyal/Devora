#!/usr/bin/env bash
# Check that the app a DMG ships is signed with a certificate, so macOS keeps its permissions across updates, rather than ad hoc.
set -euo pipefail

usage="Usage: $0 <dmg> <bundle-id>"
dmg="${1:?$usage}"
bundle_id="${2:?$usage}"

mount="$(mktemp -d)"
hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$dmg" >/dev/null
trap 'hdiutil detach "$mount" >/dev/null' EXIT

apps=("$mount"/*.app)
[[ ${#apps[@]} -eq 1 && -d "${apps[0]}" ]] || { echo "Expected one app at the root of $dmg" >&2; exit 1; }
app="${apps[0]}"
codesign --verify --deep --strict "$app"
requirement="$(codesign -d -r- "$app" 2>&1 | grep 'designated =>')"
echo "$requirement"
[[ "$requirement" == "designated => identifier \"$bundle_id\" and certificate leaf = H\""* ]] || {
	echo "$app is not signed with a certificate for $bundle_id" >&2
	exit 1
}
