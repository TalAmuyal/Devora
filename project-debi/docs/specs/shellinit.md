# Shell Init Package Spec

Package: `internal/shellinit`

## Purpose

Materialize the shell integration shipped inside the Devora app bundle.
Today that is a set of "shims": one tiny executable per Debi git shortcut that forwards to `debi <name>`, so typing the bare shortcut (e.g. `gcl`) in any session shell whose `PATH` includes the shim directory runs the Debi command.

## Constant

### GitShortcutsGroup

```go
const GitShortcutsGroup = "Git Shortcuts"
```

The command-registry group whose commands are exposed as shims.
Matches the `Group` field set on the git-shortcut commands in `internal/cli`.

## Function

### WriteShims

```go
func WriteShims(dir string, cmds []cmdinfo.Command) error
```

Writes an executable shim into `dir` for each command in `cmds` whose `Group` equals `GitShortcutsGroup`.

**Parameters:**
- `dir` — destination directory; created (including parents) if absent.
- `cmds` — command metadata, typically `cli.CommandInfos()`.

**Returns:**
- `err` — non-nil on a filesystem error (creating the directory, writing or chmod-ing a shim).

**Behavior:**
1. `os.MkdirAll(dir, 0o755)`.
2. For each command with `Group == GitShortcutsGroup`, write `dir/<name>` with the body `#!/bin/sh\nexec debi <name> "$@"\n`.
3. Each shim is written `0o755` and then `os.Chmod`-ed to `0o755` so the executable bits survive a restrictive umask.
4. Commands in other groups are skipped.

The shim relies on `debi` being on `PATH` at run time (the Devora-Ember bundle prepends `bundled-apps/`, which contains `debi`, to session-shell `PATH`).
