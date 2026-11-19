#!/usr/bin/env bash
# Checks acceptance/no-ssh, the acceptance runs' GIT_SSH_COMMAND: git over ssh fails at once, saying why, and without
# reaching the network (the URL is one Claude Code fetches its official marketplace from).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(GIT_SSH_COMMAND="'$here/no-ssh'" git ls-remote git@github.com:anthropics/claude-plugins-official.git 2>&1)
status=$?
if [ "$status" -eq 0 ] || ! grep -qF "no-ssh: tests have no internet; refused ssh git@github.com git-upload-pack 'anthropics/claude-plugins-official.git'" <<<"$out"; then
	printf 'FAIL: git over ssh must be refused by no-ssh (exit %s):\n%s\n' "$status" "$out" >&2
	exit 1
fi
echo "no-ssh: all checks passed"
