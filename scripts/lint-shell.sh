#!/usr/bin/env bash
# Lints every shell script in the repository with shellcheck (`mise run lint-shell`, part of `mise run lint`).
# The scripts are found rather than listed, so a new one is linted without anyone adding it: files ending in .sh, and
# executables whose shebang runs sh or bash, among the tracked files and the untracked ones git doesn't ignore (so a new
# script is linted before it's committed, and vendor/ and build/ never are).
# zsh scripts aren't linted: shellcheck doesn't support zsh. `--list` prints the scripts instead of linting them.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

scripts=()
while IFS= read -r file; do
	# A tracked file deleted in the working tree is no script to lint.
	[ -f "$file" ] || continue
	if [[ "$file" == *.sh ]] || { [ -x "$file" ] && head -n 1 "$file" | grep -qE '^#!.*[/ ](ba)?sh( |$)'; }; then
		scripts+=("$file")
	fi
done < <(git ls-files --cached --others --exclude-standard | sort -u)

if [ ${#scripts[@]} -eq 0 ]; then
	echo "lint-shell: found no shell scripts" >&2
	exit 1
fi
if [ "${1:-}" = --list ]; then
	printf '%s\n' "${scripts[@]}"
	exit 0
fi
shellcheck "${scripts[@]}"
echo "lint-shell: ${#scripts[@]} scripts are clean"
