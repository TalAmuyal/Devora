#!/bin/bash

set -e

APP_CONTENTS_PATH="$(cd "$(dirname "$0")/.." && pwd)"
RESOURCES_DIR="$APP_CONTENTS_PATH/Resources"

KITTY_EXECUTABLE_PATH="$RESOURCES_DIR/kitty.app/Contents/MacOS/kitty"

PATH="$RESOURCES_DIR/bundled-apps:$PATH"

cd "$HOME"

export KITTY_CONFIG_DIRECTORY="$RESOURCES_DIR/kitty-configs"

$KITTY_EXECUTABLE_PATH \
	--title "Devora" \
	zsh --login --interactive -c "debi workspace-ui"

#$KITTY_EXECUTABLE_PATH \
#	--title "Devora" \
#	debi \
#	workspace-ui
#	--working-directory "$RESOURCES_DIR" \
#	zsh --login --interactive #-c "echo \$ABCDE; sleep 5"
