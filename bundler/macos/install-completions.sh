#!/bin/bash

set -e

# Generate the debi zsh completion file from the debi binary bundled inside an installed Devora .app, and (one-time) hint the user how to wire up fpath.
# The installed app path is passed in so completions resolve for any install destination (release Devora.app, an untagged Devora-<version>.app, or Devora-dev.app).
# The generated _debi content is identical regardless of build.

if [ $# -lt 1 ]; then
	echo "Usage: $0 <installed-app-path>" >&2
	exit 1
fi

APP_PATH="$1"
DEBI_BINARY="$APP_PATH/Contents/Resources/bundled-apps/debi"
COMPLETIONS_DIR="$HOME/.zsh/completions"
COMPLETION_FILE="$COMPLETIONS_DIR/_debi"
ZSHRC="$HOME/.zshrc"
# Literal tilde is intentional: this is a grep pattern matched against the user's ~/.zshrc, where they typically write `fpath=(~/.zsh/completions ...)`.
# shellcheck disable=SC2088
FPATH_ENTRY="~/.zsh/completions"

if [ ! -x "$DEBI_BINARY" ]; then
	echo "Error: debi binary not found at $DEBI_BINARY"
	exit 1
fi

mkdir -p "$COMPLETIONS_DIR"
"$DEBI_BINARY" completion zsh > "$COMPLETION_FILE"
echo "Installed zsh completion: $COMPLETION_FILE"

if [ -f "$ZSHRC" ] && grep -q "$FPATH_ENTRY" "$ZSHRC"; then
	exit 0
fi

echo ""
echo "Notice: To enable shell completion, add the following to your ~/.zshrc (before compinit):"
echo ""
echo "    fpath=(~/.zsh/completions \$fpath)"
echo "    autoload -U compinit; compinit"
echo ""
echo "See USER_GUIDE.md, section 'Recommended: Shell Completions', for details."
