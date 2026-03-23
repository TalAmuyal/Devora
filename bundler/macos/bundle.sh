#!/bin/bash

set -e

HELPER_TEXT="Usage: $0 [--help|-h] [--dmg]

  --dmg       Create a DMG file in addition to the app bundle
  --help, -h  Show this help message and exit
"

# Parse arguments
SHOLUD_MAKE_DMG=false
while [[ $# -gt 0 ]]; do
	case "$1" in
		--help|-h)
			echo -n "$HELPER_TEXT"
			exit 0
			;;
		--dmg)
			SHOLUD_MAKE_DMG=true
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

# Define basic input paths
SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
BUNDLER_DIR=$(dirname "$SCRIPT_DIR")
REPO_ROOT=$(dirname "$BUNDLER_DIR")

# Define basic output paths
BIN_MACOS_DIR="$REPO_ROOT/bin/macOS"
OUTPUT_CONTAINER_DIR="$BIN_MACOS_DIR/Devora"
OUTPUT_DIR="$OUTPUT_CONTAINER_DIR/Devora.app"
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

# Create basic app structure
[ -d "$OUTPUT_DIR" ] && $SHOULD_CLEAN \
	&& echo -n "Cleaning previous bundle" \
	&& rm -rf "$BIN_MACOS_DIR" \
	&& while [ -e "$BIN_MACOS_DIR" ]; do sleep 0.1; done \
	&& echo -n "." \
	&& sleep 1 \
	&& echo -n "." \
	&& sync \
	&& echo -n "." \
	&& sleep 1 \
	&& echo " Done"
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

# Bundle prepared files and directories
bundle "$REPO_ROOT/USER_GUIDE.md"           "$OUTPUT_CONTAINER_DIR/."         overwrite
bundle "$REPO_ROOT/cc-simple-statusline.sh" "$OUTPUT_CONTAINER_DIR/."         overwrite
bundle "$SCRIPT_DIR/Info.plist"             "$OUTPUT_DIR/Contents/."          overwrite
bundle "$THIRD_PARTY_APPS_DIR/kitty.app"    "$RESOURCES_DIR/kitty.app"        check
bundle "$THIRD_PARTY_APPS_DIR/uv"           "$RESOURCES_DIR/uv"               check
bundle "$REPO_ROOT/kitty-configs"           "$RESOURCES_DIR/."                overwrite
bundle "$SCRIPT_DIR/bootstrap.sh"           "$ROOT_EXEC_DIR/Devora"           overwrite
bundle "$REPO_ROOT/ccc.sh"                  "$BUNDLED_APPS_DIR/ccc"           overwrite
bundle "$REPO_ROOT/project-judge/cc-plugin" "$BUNDLED_CC_PLUGINS_DIR/judge"   overwrite
bundle "$REPO_ROOT/project-judge/main.py"   "$BUNDLED_CC_PLUGINS_DIR/judge/." overwrite

# Set permissions
chmod -R 755 "$OUTPUT_DIR"

# Ensure all data is written to the disk
sync

# Make DMG file
$SHOLUD_MAKE_DMG \
	&& echo -n "Creating DMG file" \
	&& hdiutil create \
	-format UDZO \
	-imagekey zlib-level=9 \
	-srcfolder "$OUTPUT_CONTAINER_DIR" \
	-o "$BIN_MACOS_DIR/Devora.dmg"

echo ""
echo "Done"
