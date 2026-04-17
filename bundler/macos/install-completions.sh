#!/bin/bash

set -e

DEBI_BINARY="/Applications/Devora.app/Contents/Resources/bundled-apps/debi"
COMPLETIONS_DIR="$HOME/.zsh/completions"
COMPLETION_FILE="$COMPLETIONS_DIR/_debi"
ZSHRC="$HOME/.zshrc"
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
