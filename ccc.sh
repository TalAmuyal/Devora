#!/bin/bash

# Launch Claude Code with custom configuration
# The default model, model tiers, etc. are resolved from user/profile config and are injected by Devora's PTY layer.
# The ANTHROPIC_* vars are read by Claude Code natively; the DEVORA_CCC_* vars are turned into CLI flags below.

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
	echo "Error: This script must be executed, not sourced." >&2
	return 1
fi

set -e

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
SCRIPT_DIR_PARENT=$(dirname "$SCRIPT_DIR")

ARGS=()
USER_SET_EFFORT=0
USER_SET_PERMISSION_MODE=0
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

		--permission-mode|--permission-mode=*)
			USER_SET_PERMISSION_MODE=1
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

if [[ "$USER_SET_PERMISSION_MODE" -eq 0 && -n "${DEVORA_CCC_PERMISSION_MODE:-}" ]]; then
	ARGS+=(--permission-mode "$DEVORA_CCC_PERMISSION_MODE")
fi

claude "${ARGS[@]}"
