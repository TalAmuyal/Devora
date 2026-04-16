#!/bin/bash

# Launch Claude Code with custom configuration

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
	echo "Error: This script must be executed, not sourced." >&2
	return 1
fi

set -e

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
SCRIPT_DIR_PARENT=$(dirname "$SCRIPT_DIR")

ARGS=("$@")

# ccc --debug hooks
PLUGINS_DIR="$SCRIPT_DIR_PARENT/cc-plugins"
if [ -d "$PLUGINS_DIR" ]; then
	for plugin in "$PLUGINS_DIR"/*; do
		[ -d "$plugin" ] && ARGS+=("--plugin-dir" "$plugin")
	done
fi

# Known model names
CC_HAIKU_4_5="claude-haiku-4-5-20251001"
CC_SONNET_4_6="claude-sonnet-4-6"
CC_OPUS_4_6="claude-opus-4-6"
CC_OPUS_4_7="claude-opus-4-7"

export ANTHROPIC_DEFAULT_OPUS_MODEL="$CC_OPUS_4_7"
export ANTHROPIC_DEFAULT_SONNET_MODEL="$CC_OPUS_4_7"
export ANTHROPIC_DEFAULT_HAIKU_MODEL="$CC_SONNET_4_6"

export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
export CLAUDE_CODE_ENABLE_TELEMETRY=0
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1

CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1 claude --effort max "${ARGS[@]}"
