#!/bin/bash

set -eEo pipefail

HELPER_TEXT="Usage: $0 [--help|-h] [--open]

  --open      Open the built app after bundling
  --help, -h  Show this help message and exit
"

# Parse arguments
SHOULD_OPEN=false
while [[ $# -gt 0 ]]; do
	case "$1" in
		--help|-h)
			echo -n "$HELPER_TEXT"
			exit 0
			;;
		--open)
			SHOULD_OPEN=true
			shift
			;;
		*)
			echo "Unknown argument: $1"
			echo ""
			echo -n "$HELPER_TEXT"
			exit 1
			;;
	esac
done

# Define paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUNDLER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$BUNDLER_DIR/.." && pwd)"
EMBER_DIR="$REPO_ROOT/project-ember"
THIRD_PARTY_APPS_DIR="$BUNDLER_DIR/macos/3rd-party-apps"

# Compute effective version
BASE_VERSION=$(cat "$REPO_ROOT/VERSION")
if git -C "$REPO_ROOT" describe --exact-match --tags --match 'v*' HEAD >/dev/null 2>&1; then
	EFFECTIVE_VERSION="$BASE_VERSION"
else
	LAST_TAG=$(git -C "$REPO_ROOT" describe --tags --match 'v*' --abbrev=0 2>/dev/null || echo "")
	if [ -n "$LAST_TAG" ]; then
		COMMIT_COUNT=$(git -C "$REPO_ROOT" rev-list --count "${LAST_TAG}..HEAD")
	else
		COMMIT_COUNT=$(git -C "$REPO_ROOT" rev-list --count HEAD)
	fi
	EFFECTIVE_VERSION="${BASE_VERSION}-dev.${COMMIT_COUNT}"
fi

echo "Effective version: $EFFECTIVE_VERSION"

# --- Step 1: Build Go binaries ---

echo ""
echo "==> Building Go binaries..."

GOOS=darwin \
	GOARCH=arm64 \
	mise \
	-C "$REPO_ROOT/project-debi" \
	exec \
	-- \
	go build \
	-o "$REPO_ROOT/project-debi/debi"

GOOS=darwin \
	GOARCH=arm64 \
	mise \
	-C "$REPO_ROOT/project-status-line" \
	exec \
	-- \
	go build \
	-o "$REPO_ROOT/project-status-line/cc-simple-statusline"

echo "    Go binaries built."

# --- Step 2: Build Ember app ---

echo ""
echo "==> Building Devora Ember..."

(cd "$EMBER_DIR" && mise exec -- cargo tauri build --bundles app)

echo "    Devora Ember built."

# --- Step 3: Locate the .app bundle ---

APP_DIR="$EMBER_DIR/src-tauri/target/release/bundle/macos/Devora Ember.app"

if [ ! -d "$APP_DIR" ]; then
	echo "Error: Built app not found at: $APP_DIR"
	exit 1
fi

# --- Step 4: Copy bundled resources into the .app ---

echo ""
echo "==> Bundling resources into Devora Ember.app..."

RESOURCES_DIR="$APP_DIR/Contents/Resources"
BUNDLED_APPS_DIR="$RESOURCES_DIR/bundled-apps"
BUNDLED_CC_PLUGINS_DIR="$RESOURCES_DIR/cc-plugins"

mkdir -p "$BUNDLED_APPS_DIR" "$BUNDLED_CC_PLUGINS_DIR"

# Helper: copy a resource into the bundle, warning (not failing) if the source is missing
bundle() {
	local src="$1"
	local dest="$2"
	local desc="${src##*/}"

	if [ ! -e "$src" ]; then
		echo "    Warning: $desc not found at $src -- skipping"
		return 0
	fi

	cp -R "$src" "$dest"
	sync
}

# Helper: copy a resource that is required -- fail if missing
bundle_required() {
	local src="$1"
	local dest="$2"
	local desc="${src##*/}"

	if [ ! -e "$src" ]; then
		echo "    Error: required resource $desc not found at $src"
		exit 1
	fi

	cp -R "$src" "$dest"
	sync
}

# Bundled apps
bundle_required "$REPO_ROOT/project-debi/debi"                "$BUNDLED_APPS_DIR/debi"
bundle_required "$REPO_ROOT/ccc.sh"                            "$BUNDLED_APPS_DIR/ccc"
bundle          "$REPO_ROOT/project-crit-integration/bin/crit" "$BUNDLED_APPS_DIR/crit"

# CC plugins
bundle_required "$REPO_ROOT/project-judge/cc-plugin"           "$BUNDLED_CC_PLUGINS_DIR/judge"
bundle_required "$REPO_ROOT/project-judge/main.py"             "$BUNDLED_CC_PLUGINS_DIR/judge/."
bundle_required "$REPO_ROOT/project-detached-flow/cc-plugin"   "$BUNDLED_CC_PLUGINS_DIR/detached-flow"
bundle_required "$REPO_ROOT/project-team-work/cc-plugin"       "$BUNDLED_CC_PLUGINS_DIR/team-work"
bundle          "$REPO_ROOT/project-crit-integration/cc-plugin" "$BUNDLED_CC_PLUGINS_DIR/crit"

# Third-party: original-crit binary and crit cc-plugin (from downloaded 3rd-party deps)
bundle_required "$THIRD_PARTY_APPS_DIR/original-crit"                   "$BUNDLED_APPS_DIR/original-crit"
bundle_required "$THIRD_PARTY_APPS_DIR/claude-code"                     "$BUNDLED_CC_PLUGINS_DIR/crit"

# Third-party: uv (Python package manager, needed by Judge)
bundle_required "$THIRD_PARTY_APPS_DIR/uv"                              "$RESOURCES_DIR/uv"
if [ -f "$BUNDLER_DIR/uv-license.txt" ]; then
	bundle "$BUNDLER_DIR/uv-license.txt"                       "$RESOURCES_DIR/."
fi
if [ -f "$BUNDLER_DIR/crit-license.txt" ]; then
	bundle "$BUNDLER_DIR/crit-license.txt"                     "$RESOURCES_DIR/."
fi

# Kitty configs (theme file, etc.)
mkdir -p "$RESOURCES_DIR/kitty-configs"
bundle "$REPO_ROOT/kitty-configs/current-theme.conf"           "$RESOURCES_DIR/kitty-configs/current-theme.conf"

# Docs and version
bundle_required "$REPO_ROOT/USER_GUIDE.md"                     "$RESOURCES_DIR/."
bundle_required "$REPO_ROOT/CHANGELOG.md"                      "$RESOURCES_DIR/."
echo -n "$EFFECTIVE_VERSION" >                                  "$RESOURCES_DIR/VERSION"

# CC status line
bundle_required "$REPO_ROOT/project-status-line/cc-simple-statusline" "$RESOURCES_DIR/cc-simple-statusline"

# Set permissions
chmod -R 755 "$APP_DIR"

# Ensure all data is written to disk
sync

echo "    Resources bundled."

# --- Step 5: Open if requested ---

$SHOULD_OPEN && open "$APP_DIR"

# --- Done ---

echo ""
echo "Done. App bundle at:"
echo "  $APP_DIR"
