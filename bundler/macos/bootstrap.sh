#!/bin/bash

set -eEo pipefail

LOG_FILE="/tmp/devora-bootstrap-$(date +%Y%m%d-%H%M%S).log"

on_error() {
	local exit_code=$?
	{
		echo "=== Devora Bootstrap Error ==="
		echo "Timestamp: $(date)"
		echo "Script: $0"
		echo "Line: $1"
		echo "Command: $BASH_COMMAND"
		echo "Exit code: $exit_code"
		echo ""
		echo "Environment:"
		echo "  APP_CONTENTS_PATH=${APP_CONTENTS_PATH:-<unset>}"
		echo "  RESOURCES_DIR=${RESOURCES_DIR:-<unset>}"
		echo "  KITTY_EXECUTABLE_PATH=${KITTY_EXECUTABLE_PATH:-<unset>}"
		echo "  KITTY_CONFIG_DIRECTORY=${KITTY_CONFIG_DIRECTORY:-<unset>}"
		echo "  HOME=$HOME"
		echo "  PATH=$PATH"
	} >"$LOG_FILE" 2>&1
	echo "Devora bootstrap failed (exit code $exit_code). Log: $LOG_FILE" >&2
}

trap 'on_error $LINENO' ERR

export SHELL="${SHELL:-/bin/zsh}"

APP_CONTENTS_PATH="$(cd "$(dirname "$0")/.." && pwd)"
RESOURCES_DIR="$APP_CONTENTS_PATH/Resources"
CC_PLUGINS_DIR="$RESOURCES_DIR/cc-plugins"

KITTY_EXECUTABLE_PATH="$RESOURCES_DIR/kitty.app/Contents/MacOS/kitty"

PATH="$RESOURCES_DIR/bundled-apps:$PATH"
if [ -d "$CC_PLUGINS_DIR" ]; then
	for plugin in "$CC_PLUGINS_DIR"/*; do
		[ -d "$plugin/bin" ] && PATH="$plugin/bin:$PATH"
	done
fi

cd "$HOME"

[ ! -e ~/.hushlogin ] && touch ~/.hushlogin

export KITTY_CONFIG_DIRECTORY="$RESOURCES_DIR/kitty-configs"
export DEVORA_RESOURCES_DIR="$RESOURCES_DIR"

$KITTY_EXECUTABLE_PATH \
	--title "Devora" \
	"$SHELL" -l -i -c "\"$RESOURCES_DIR/bundled-apps/debi\" workspace-ui"
