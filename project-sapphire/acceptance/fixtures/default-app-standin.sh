#!/bin/sh
# A stand-in for a terminal's Default app (`terminal.default-app`): it reports what a program started in a Sapphire
# terminal sees (Devora's environment, its directory), then leaves a shell so the tab stays open.
printf 'STANDIN pty=%s port=%s cwd=%s\n' "$DEVORA_PTY_ID" "$DEVORA_IPC_PORT" "$PWD"
exec /bin/sh
