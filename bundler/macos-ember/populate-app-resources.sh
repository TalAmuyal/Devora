#!/bin/bash

set -eEo pipefail

HELPER_TEXT="Usage: $0 <app-dir> --version <ver>

Copies all resources into an existing Ember .app bundle.
This is the single source of truth for what goes into the bundle.

Arguments:
  <app-dir>            Path to the .app directory to populate
  --version <ver>      Version string to write into the bundle
  --help, -h           Show this help message and exit
"

# Parse arguments
APP_DIR=""
VERSION=""
while [[ $# -gt 0 ]]; do
	case "$1" in
		--help|-h)
			echo -n "$HELPER_TEXT"
			exit 0
			;;
		--version)
			if [[ -z "${2:-}" ]]; then
				echo "Error: --version requires a value"
				exit 1
			fi
			VERSION="$2"
			shift 2
			;;
		*)
			if [[ -z "$APP_DIR" ]]; then
				APP_DIR="$1"
				shift
			else
				echo "Error: unexpected argument: $1"
				echo ""
				echo -n "$HELPER_TEXT"
				exit 1
			fi
			;;
	esac
done

if [[ -z "$APP_DIR" ]]; then
	echo "Error: <app-dir> is required"
	echo ""
	echo -n "$HELPER_TEXT"
	exit 1
fi

if [[ -z "$VERSION" ]]; then
	echo "Error: --version is required"
	echo ""
	echo -n "$HELPER_TEXT"
	exit 1
fi

# Define paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUNDLER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$BUNDLER_DIR/.." && pwd)"
THIRD_PARTY_APPS_DIR="$BUNDLER_DIR/macos/3rd-party-apps"

# Validate app directory
if [[ ! -d "$APP_DIR" ]]; then
	echo "Error: app directory not found: $APP_DIR"
	exit 1
fi

if [[ ! -d "$APP_DIR/Contents" ]]; then
	echo "Error: $APP_DIR does not contain a Contents/ directory"
	exit 1
fi

RESOURCES_DIR="$APP_DIR/Contents/Resources"
BUNDLED_APPS_DIR="$RESOURCES_DIR/bundled-apps"
BUNDLED_CC_PLUGINS_DIR="$RESOURCES_DIR/cc-plugins"

mkdir -p "$BUNDLED_APPS_DIR" "$BUNDLED_CC_PLUGINS_DIR"

# Helper: copy a required resource into the bundle -- fail if missing
copy_required() {
	local src="$1"
	local dest="$2"
	local desc="${src##*/}"

	if [[ ! -e "$src" ]]; then
		echo "Error: required resource $desc not found at $src"
		exit 1
	fi

	cp -R "$src" "$dest"
	sync
	sleep 0.07
}

# --- Bundled apps ---

copy_required "$REPO_ROOT/ccc.sh"                            "$BUNDLED_APPS_DIR/ccc"
copy_required "$REPO_ROOT/project-crit-integration/bin/crit"  "$BUNDLED_APPS_DIR/crit"
copy_required "$THIRD_PARTY_APPS_DIR/original-crit"           "$BUNDLED_APPS_DIR/original-crit"
copy_required "$REPO_ROOT/project-debi/debi"                  "$BUNDLED_APPS_DIR/debi"

# --- CC plugins ---

copy_required "$REPO_ROOT/project-judge/cc-plugin"            "$BUNDLED_CC_PLUGINS_DIR/judge"
copy_required "$REPO_ROOT/project-judge/main.py"              "$BUNDLED_CC_PLUGINS_DIR/judge/."
copy_required "$REPO_ROOT/project-detached-flow/cc-plugin"    "$BUNDLED_CC_PLUGINS_DIR/detached-flow"
copy_required "$REPO_ROOT/project-team-work/cc-plugin"        "$BUNDLED_CC_PLUGINS_DIR/team-work"
copy_required "$REPO_ROOT/project-crit-integration/cc-plugin" "$BUNDLED_CC_PLUGINS_DIR/crit-integration"
copy_required "$THIRD_PARTY_APPS_DIR/claude-code"             "$BUNDLED_CC_PLUGINS_DIR/crit"

# --- Resources ---

copy_required "$THIRD_PARTY_APPS_DIR/uv"                              "$RESOURCES_DIR/uv"
copy_required "$BUNDLER_DIR/uv-license.txt"                            "$RESOURCES_DIR/."
copy_required "$BUNDLER_DIR/crit-license.txt"                          "$RESOURCES_DIR/."

mkdir -p "$RESOURCES_DIR/kitty-configs"
copy_required "$REPO_ROOT/kitty-configs/current-theme.conf"            "$RESOURCES_DIR/kitty-configs/current-theme.conf"

copy_required "$REPO_ROOT/USER_GUIDE.md"                               "$RESOURCES_DIR/."
copy_required "$REPO_ROOT/CHANGELOG.md"                                "$RESOURCES_DIR/."
copy_required "$REPO_ROOT/project-status-line/cc-simple-statusline"    "$RESOURCES_DIR/cc-simple-statusline"

# --- Version ---

echo -n "$VERSION" > "$RESOURCES_DIR/VERSION"

# --- Finalize ---

chmod -R 755 "$APP_DIR"
sync
