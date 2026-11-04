#!/usr/bin/env bash
# Build the signed app used by source installs and release DMGs.
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
suffix=""
prebuilt=false
while [[ $# -gt 0 ]]; do
	case "$1" in
		--suffix) suffix="${2:?--suffix requires a value}"; shift 2 ;;
		--prebuilt) prebuilt=true; shift ;;
		--help|-h) echo "Usage: $0 [--suffix <suffix>] [--prebuilt (reuse Sapphire build/test-tools)]"; exit 0 ;;
		*) echo "Unknown argument: $1" >&2; exit 1 ;;
	esac
done
case "$suffix" in */*|*\\*|.|..) echo "Invalid install suffix" >&2; exit 1 ;; esac
cd "$repo_root"
version="$(bundler/macos-ember/effective-version.sh)"
if [[ "$prebuilt" == true ]]; then
	# The CI job already built these through Sapphire's test-tools task.
	test -x project-sapphire/build/test-tools/debi
	test -x project-core/target/release/debi
	cp project-sapphire/build/test-tools/debi project-debi/debi
	# These helpers are not otherwise built by Sapphire's tests.
	mise run -C project-status-line build
	mise run -C project-judgess build
else
	mise run build
	mise run -C project-sapphire build
fi
output="$repo_root/project-sapphire/build/bundle"
app="$output/Devora Sapphire.app"
mkdir -p "$output"
rm -rf "$app"
ditto "$repo_root/project-sapphire/build/DerivedData/Build/Products/Release/Devora Sapphire.app" "$app"
bundler/macos-ember/populate-app-resources.sh "$app" --app sapphire --version "$version"
name="Devora Sapphire"
if [[ -n "$suffix" ]]; then
	name="$name-$suffix"
	printf '%s' "$suffix" > "$app/Contents/Resources/INSTALL_SUFFIX"
elif [[ "$version" == *-dev.* ]]; then
	name="$name-$version"
fi
plutil -replace CFBundleName -string "$name" "$app/Contents/Info.plist"
plutil -replace CFBundleDisplayName -string "$name" "$app/Contents/Info.plist"
# Sign every nested Mach-O first (including future plugin executables), then seal the app.
while IFS= read -r -d '' binary; do
	if [[ "$(file -b "$binary")" == Mach-O* ]]; then
		codesign --force --sign - "$binary"
	fi
done < <(find "$app/Contents" -type f -print0)
codesign --force --sign - --options runtime "$app"
codesign --verify --deep --strict "$app"
echo "Bundled $app ($version)"
