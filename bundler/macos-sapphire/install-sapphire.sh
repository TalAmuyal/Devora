#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
install_dir=/Applications
suffix=""
while [[ $# -gt 0 ]]; do
	case "$1" in
		--path|-p) install_dir="${2:?--path requires a directory}"; shift 2 ;;
		--suffix|-s) suffix="${2:?--suffix requires a value}"; shift 2 ;;
		--help|-h) echo "Usage: $0 [--path <directory>] [--suffix <suffix>]"; exit 0 ;;
		*) echo "Unknown argument: $1" >&2; exit 1 ;;
	esac
done
[[ -d "$install_dir" ]] || { echo "Install directory does not exist: $install_dir" >&2; exit 1; }
install_dir="$(cd "$install_dir" && pwd -P)"
if [[ -n "$suffix" ]]; then
	"$repo_root/bundler/macos-sapphire/bundle-sapphire.sh" --suffix "$suffix"
else
	"$repo_root/bundler/macos-sapphire/bundle-sapphire.sh"
fi
app="$repo_root/project-sapphire/build/bundle/Devora Sapphire.app"
name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleDisplayName' "$app/Contents/Info.plist")"
destination="${install_dir%/}/$name.app"
if [[ "$destination" == "$app" ]]; then echo "Install directory must differ from the bundle output" >&2; exit 1; fi
rm -rf "$destination"
ditto "$app" "$destination"
codesign --verify --deep --strict "$destination"
echo "Installed to $destination"
