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

APP_DIR="$EMBER_DIR/src-tauri/target/release/bundle/macos/Devora-Ember.app"

if [ ! -d "$APP_DIR" ]; then
	echo "Error: Built app not found at: $APP_DIR"
	exit 1
fi

# --- Step 4: Populate app resources ---

echo ""
echo "==> Bundling resources into Devora Ember.app..."

"$SCRIPT_DIR/populate-app-resources.sh" "$APP_DIR" --version "$EFFECTIVE_VERSION"

echo "    Resources bundled."

# --- Step 5: Open if requested ---

$SHOULD_OPEN && open "$APP_DIR"

# --- Done ---

echo ""
echo "Done. App bundle at:"
echo "  $APP_DIR"
