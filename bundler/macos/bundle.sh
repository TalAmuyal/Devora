#!/bin/bash

set -e

HELPER_TEXT="Usage: $0 [--help|-h] [--dmg] [--dev]

  --dev       Build \"Dev-Devora\" variant (can coexist with production app)
  --dmg       Create a DMG file in addition to the app bundle
  --help, -h  Show this help message and exit
"

# Parse arguments
SHOULD_MAKE_DMG=false
IS_DEV_MODE=false
while [[ $# -gt 0 ]]; do
	case "$1" in
		--help|-h)
			echo -n "$HELPER_TEXT"
			exit 0
			;;
		--dmg)
			SHOULD_MAKE_DMG=true
			shift
			;;
		--dev)
			IS_DEV_MODE=true
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

# Derive app identity from mode
if $IS_DEV_MODE; then
	APP_NAME="Dev-Devora"
	APP_IDENTIFIER="com.devora-org.devora-dev"
	BIN_MACOS_DIR_SUFFIX="-dev"
else
	APP_NAME="Devora"
	APP_IDENTIFIER="com.devora-org.devora"
	BIN_MACOS_DIR_SUFFIX=""
fi

# Define basic input paths
SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
BUNDLER_DIR=$(dirname "$SCRIPT_DIR")
REPO_ROOT=$(dirname "$BUNDLER_DIR")

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

# Define basic output paths
BIN_MACOS_DIR="$REPO_ROOT/bin/macOS$BIN_MACOS_DIR_SUFFIX"
OUTPUT_CONTAINER_DIR="$BIN_MACOS_DIR/$APP_NAME"
OUTPUT_DIR="$OUTPUT_CONTAINER_DIR/$APP_NAME.app"
ROOT_EXEC_DIR="$OUTPUT_DIR/Contents/MacOS"
RESOURCES_DIR="$OUTPUT_DIR/Contents/Resources"
BUNDLED_APPS_DIR="$RESOURCES_DIR/bundled-apps"
BUNDLED_CC_PLUGINS_DIR="$RESOURCES_DIR/cc-plugins"

bundle() {
	local src="$1"
	local dest="$2"
	local mode="$3"
	local desc="${src##*/}"

	[ ! -e "$src" ] && { echo "Error: $desc not found at $src"; exit 1; }

	case "$mode" in
		check)
			if [ ! -e "$dest" ]; then
				cp -R "$src" "$dest"
			fi
			;;
		overwrite)
			cp -R "$src" "$dest"
			;;
		"")
			echo "Error: mode not set (must be 'check' or 'overwrite')"
			exit 1
			;;
		*)
			echo "Error: invalid mode '$mode' (must be 'check' or 'overwrite')"
			exit 1
			;;
	esac

	sync
	sleep 0.07
}

# Clean previous build
rm -rf "$BIN_MACOS_DIR"
# Make sure the directory is fully removed before proceeding
while [ -e "$BIN_MACOS_DIR" ]; do sleep 0.1; done
sleep 0.5

# Create basic app structure
mkdir -p "$ROOT_EXEC_DIR" "$RESOURCES_DIR" "$BUNDLED_APPS_DIR" "$BUNDLED_CC_PLUGINS_DIR"

# Prepare third-party apps
THIRD_PARTY_APPS_DIR="$SCRIPT_DIR/3rd-party-apps"

# Prepare Applications symlink
$(cd "$OUTPUT_CONTAINER_DIR" && [ -e "Applications" ] || ln -s /Applications Applications)

# Bundle Debi
GOOS=darwin \
	GOARCH=arm64 \
	mise \
	-C "$REPO_ROOT/project-debi" \
	exec \
	-- \
	go build \
	-o "$BUNDLED_APPS_DIR/debi"

# Bundle icons
[ ! -f "$RESOURCES_DIR/app.icns" ] &&
	"$SCRIPT_DIR/gen-icon.sh" \
	--base-icon "$REPO_ROOT/base-icon.png" \
	--output-dir "$RESOURCES_DIR"

# Bundle Status-line
GOOS=darwin \
	GOARCH=arm64 \
	mise \
	-C "$REPO_ROOT/project-status-line" \
	exec \
	-- \
	go build \
	-o "$OUTPUT_CONTAINER_DIR/cc-simple-statusline"

# Bundle prepared files and directories
bundle "$REPO_ROOT/USER_GUIDE.md"           "$OUTPUT_CONTAINER_DIR/."         overwrite
bundle "$REPO_ROOT/USER_GUIDE.md"           "$RESOURCES_DIR/."                overwrite
bundle "$REPO_ROOT/CHANGELOG.md"            "$OUTPUT_CONTAINER_DIR/."         overwrite
bundle "$REPO_ROOT/CHANGELOG.md"            "$RESOURCES_DIR/."                overwrite
echo -n "$EFFECTIVE_VERSION"          >     "$RESOURCES_DIR/VERSION"
bundle "$SCRIPT_DIR/Info.plist"             "$OUTPUT_DIR/Contents/."          overwrite
bundle "$THIRD_PARTY_APPS_DIR/kitty.app"    "$RESOURCES_DIR/kitty.app"        check
bundle "$BUNDLER_DIR/kitty-license.txt"     "$RESOURCES_DIR/."                overwrite
bundle "$BUNDLER_DIR/uv-license.txt"        "$RESOURCES_DIR/."                overwrite
bundle "$THIRD_PARTY_APPS_DIR/uv"           "$RESOURCES_DIR/uv"               check
bundle "$THIRD_PARTY_APPS_DIR/glow"          "$BUNDLED_APPS_DIR/glow"         check
bundle "$REPO_ROOT/kitty-configs"           "$RESOURCES_DIR/."                overwrite
bundle "$SCRIPT_DIR/bootstrap.sh"           "$ROOT_EXEC_DIR/."                overwrite
bundle "$REPO_ROOT/ccc.sh"                  "$BUNDLED_APPS_DIR/ccc"           overwrite
bundle "$REPO_ROOT/project-judge/cc-plugin" "$BUNDLED_CC_PLUGINS_DIR/judge"   overwrite
bundle "$REPO_ROOT/project-judge/main.py"   "$BUNDLED_CC_PLUGINS_DIR/judge/." overwrite

# Patch Info.plist with effective version
sed -i '' "s|<string>1\.0</string>|<string>$EFFECTIVE_VERSION</string>|g" "$OUTPUT_DIR/Contents/Info.plist"

# Customize bundled files for dev mode
if $IS_DEV_MODE; then
	# Patch Info.plist: app name, executable name (both are <string>Devora</string>), and identifier
	sed -i '' \
		-e 's|<string>Devora</string>|<string>'"$APP_NAME"'</string>|' \
		-e 's|<string>com.devora-org.devora</string>|<string>'"$APP_IDENTIFIER"'</string>|' \
		"$OUTPUT_DIR/Contents/Info.plist"

	# Patch bootstrap: log prefix, error messages, and window title
	sed -i '' \
		-e 's|/tmp/devora-bootstrap-|/tmp/devora-dev-bootstrap-|' \
		-e 's|Devora Bootstrap Error|Dev-Devora Bootstrap Error|' \
		-e 's|Devora bootstrap failed|Dev-Devora bootstrap failed|' \
		-e 's|--title "Devora"|--title "Dev-Devora"|' \
		"$ROOT_EXEC_DIR/bootstrap.sh"

	# Patch kitty.conf: tab title and socket path
	sed -i '' \
		-e 's|--tab-title "Devora"|--tab-title "Dev-Devora"|' \
		-e 's|devora-kitty\.sock|devora-kitty-dev.sock|' \
		"$RESOURCES_DIR/kitty-configs/kitty.conf"
fi

# Set permissions
chmod -R 755 "$OUTPUT_DIR"

# Ensure all data is written to the disk
sync

# Make DMG file
$SHOULD_MAKE_DMG \
	&& echo -n "Creating DMG file" \
	&& hdiutil create \
	-format UDZO \
	-imagekey zlib-level=9 \
	-srcfolder "$OUTPUT_CONTAINER_DIR" \
	-o "$BIN_MACOS_DIR/${APP_NAME}_${EFFECTIVE_VERSION}.dmg"

echo "Done"
