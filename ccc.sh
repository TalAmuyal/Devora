#!/bin/bash

# Launch Claude Code with custom configuration

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
	echo "Error: This script must be executed, not sourced." >&2
	return 1
fi

set -e

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
SCRIPT_DIR_PARENT=$(dirname "$SCRIPT_DIR")

ARGS=()
while [[ $# -gt 0 ]]; do
	case "$1" in
		-u|--update|update)
			claude --update
			exit 0
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

# Model tiers (ANTHROPIC_DEFAULT_*_MODEL) and the effort level (DEVORA_CCC_EFFORT) are resolved from user/profile config and injected by Devora's PTY layer; ccc no longer hardcodes them.
# Run outside Devora (no PTY) they're simply unset and Claude Code uses its own defaults.

export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="${CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC:-1}"
export CLAUDE_CODE_ENABLE_TELEMETRY="${CLAUDE_CODE_ENABLE_TELEMETRY:-0}"
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS="${CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS:-1}"

# Pass the configured effort level through to Claude Code, if one is set.
EFFORT_ARGS=()
if [[ -n "${DEVORA_CCC_EFFORT:-}" ]]; then
	EFFORT_ARGS=(--effort "$DEVORA_CCC_EFFORT")
fi

claude "${EFFORT_ARGS[@]}" --permission-mode plan "${ARGS[@]}"
