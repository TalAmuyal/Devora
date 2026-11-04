#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
output="$repo_root/project-sapphire/build/bundle"
app="$output/Devora Sapphire.app"
test -d "$app"
codesign --verify --deep --strict "$app"
staging="$(mktemp -d "$output/dmg-staging.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
ditto "$app" "$staging/Devora Sapphire.app"
ln -s /Applications "$staging/Applications"
rm -f "$output/Devora-Sapphire.dmg"
"$repo_root/bundler/hdiutil-create-retry.sh" -format UDZO -imagekey zlib-level=9 \
	-volname 'Devora Sapphire' -srcfolder "$staging" -o "$output/Devora-Sapphire.dmg"
echo "DMG created: $output/Devora-Sapphire.dmg"
