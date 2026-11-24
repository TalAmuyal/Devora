#!/bin/bash

set -eEo pipefail
# Ember runs on macOS 26 and newer (tauri.conf.json's minimumSystemVersion). Everything built here, the helpers bundled
# with it included, targets that: the C code the crates compile otherwise targets the build machine's SDK.
export MACOSX_DEPLOYMENT_TARGET=26.0

HELPER_TEXT="Usage: $0 [--help|-h] [--open] [--name <name>] [--suffix <suffix>]

  --open             Open the built app after bundling
  --name <name>      App name for the menu bar, Dock and Cmd+Tab (default: Devora Ember)
  --suffix <suffix>  Install suffix, shown in the window title
  --help, -h         Show this help message and exit
"

# Parse arguments
SHOULD_OPEN=false
APP_NAME=""
SUFFIX=""
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
		--name)
			APP_NAME="${2:?--name requires a value}"
			shift 2
			;;
		--suffix)
			SUFFIX="${2:?--suffix requires a value}"
			shift 2
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

EFFECTIVE_VERSION="$("$SCRIPT_DIR/effective-version.sh")"

echo "Effective version: $EFFECTIVE_VERSION"

# --- Build Go binaries ---

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

# --- Build the Rust debi ---

echo ""
echo "==> Building the Rust debi..."

mise \
	-C "$REPO_ROOT/project-core" \
	exec \
	-- \
	cargo build --release -p debi

echo "    Rust debi built."

# --- Build Judgess ---

echo ""
echo "==> Building Judgess..."

mise \
	-C "$REPO_ROOT/project-judgess" \
	exec \
	-- \
	cargo build --release

echo "    Judgess built."

# --- Build Ember app ---

echo ""
echo "==> Building Devora Ember..."

(cd "$EMBER_DIR" && mise exec -- cargo tauri build --bundles app)

echo "    Devora Ember built."

# --- Locate the .app bundle ---

APP_DIR="$EMBER_DIR/src-tauri/target/release/bundle/macos/Devora Ember.app"

if [ ! -d "$APP_DIR" ]; then
	echo "Error: Built app not found at: $APP_DIR"
	exit 1
fi

# --- Populate app resources ---

echo ""
echo "==> Bundling resources into Devora Ember.app..."

"$SCRIPT_DIR/populate-app-resources.sh" "$APP_DIR" --version "$EFFECTIVE_VERSION"

echo "    Resources bundled."

if [[ -n "$SUFFIX" ]]; then
	# Read by Ember for the window title.
	printf "%s" "$SUFFIX" > "$APP_DIR/Contents/Resources/INSTALL_SUFFIX"
fi
if [[ -n "$APP_NAME" ]]; then
	plutil -replace CFBundleName -string "$APP_NAME" "$APP_DIR/Contents/Info.plist"
	plutil -replace CFBundleDisplayName -string "$APP_NAME" "$APP_DIR/Contents/Info.plist"
fi

# --- Sign ---

echo ""
echo "==> Signing Devora Ember.app..."

"$BUNDLER_DIR/sign-macos-app.sh" "$APP_DIR"

# --- Done ---

echo ""
echo "Done. App bundle at:"
echo "  $APP_DIR"

if [[ "$SHOULD_OPEN" == true ]]; then
	open "$APP_DIR"
fi
