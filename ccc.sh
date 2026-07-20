#!/bin/bash

# Launch Claude Code with custom configuration
# Model tiers (ANTHROPIC_DEFAULT_*_MODEL) and the effort level (DEVORA_CCC_EFFORT) are resolved from user/profile config and injected by Devora's PTY layer

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
	echo "Error: This script must be executed, not sourced." >&2
	return 1
fi

set -e

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
SCRIPT_DIR_PARENT=$(dirname "$SCRIPT_DIR")

ARGS=()
USER_SET_EFFORT=0
while [[ $# -gt 0 ]]; do
	case "$1" in
		-u|--update|update)
			claude --update
			exit 0
		;;

		--effort|--effort=*)
			USER_SET_EFFORT=1
			ARGS+=("$1")
			;;

		*)
			ARGS+=("$1")
			;;
	esac
	shift
done

# ccc --debug hooks
PLUGINS_DIR="$SCRIPT_DIR_PARENT/cc-plugins"
if [ -d "$PLUGINS_DIR" ]; then
	for plugin in "$PLUGINS_DIR"/*; do
		[ -d "$plugin" ] && ARGS+=("--plugin-dir" "$plugin")
	done
fi

export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="${CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC:-1}"
export CLAUDE_CODE_ENABLE_TELEMETRY="${CLAUDE_CODE_ENABLE_TELEMETRY:-0}"
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS="${CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS:-1}"

if [[ "$USER_SET_EFFORT" -eq 0 && -n "${DEVORA_CCC_EFFORT:-}" ]]; then
	ARGS+=(--effort "$DEVORA_CCC_EFFORT")
fi

claude --permission-mode plan "${ARGS[@]}"
