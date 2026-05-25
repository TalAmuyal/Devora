# Invoke Error Visibility

## Problem

Tauri `invoke()` errors in the Devora-Ember frontend are silently swallowed by `console.error()`. Users never see these errors because they have no reason to open the browser console. This makes Tauri ACL denials, backend panics, and other invoke failures invisible.

## Where the pattern occurs

Every `invoke()` call in the frontend catches errors with `console.error` and either discards the error or degrades silently.

### `WorkspaceHub.ts`

| Line | Invoke call | Consequence of silent failure |
|------|-------------|-------------------------------|
| 115-123 | `list_profiles` | Hub loads with no profiles; user sees "No profiles configured" with no explanation |
| 392-399 | `list_workspaces` | Workspace list appears empty |
| 838-844 | `get_registered_repos` | "New Task" form shows no repos to select |
| 912-925 | `create_workspace` | User clicks "Create", nothing happens |
| 979-989 | `save_profiling_report` | "Save loading latencies" button does nothing |

### `TerminalPane.ts`

| Line | Invoke call | Consequence of silent failure |
|------|-------------|-------------------------------|
| 153-157 | `create_pty` | Terminal pane shows error text (this one does write to the terminal, so partially visible) |
| 167 | `write_pty` | Keystrokes silently dropped |
| 172-173 | `resize_pty` | Terminal dimensions silently wrong |

### `main.ts`

| Line | Invoke call | Consequence of silent failure |
|------|-------------|-------------------------------|
| 109-114 | `get_user_guide_path` | User Guide shortcut does nothing |

## Root cause example: ACL denial

The `save_profiling_report` command was added to Rust code (`commands.rs`) and registered in the invoke handler (`lib.rs`) but was initially missing from `build.rs` and `capabilities/main.json`. Tauri v2's ACL system denied the call at runtime. The only indication was a `console.error` message the user never sees.

## The ACL registration pattern

Every new Tauri command requires updates in three places. Missing any one causes a silent runtime failure.

1. **Rust implementation + invoke handler** -- Define the `#[tauri::command]` function in `src-tauri/src/commands.rs` (or another module) and register it in the `tauri::generate_handler![]` macro in `src-tauri/src/lib.rs`.

2. **Build manifest** -- Add the command name string to the `.commands(&[...])` array in `src-tauri/build.rs`. This generates the permission TOML files that the ACL system reads.

3. **Capability permissions** -- Add `"allow-<command-name>"` (kebab-case) to the `permissions` array in `src-tauri/capabilities/main.json`.

If step 1 is done but step 2 or 3 is missing, the command compiles and appears to exist but Tauri's ACL layer rejects every call at runtime.

## Recommended approach

### User-facing error display

Add a global mechanism for surfacing invoke errors to the user. Options:

- A toast/notification system that briefly shows the error message.
- A status bar area that displays the most recent error.
- A wrapper around `invoke()` that handles errors consistently instead of relying on each call site to do it.

The key requirement is that invoke failures become visible without the browser console.

### Lint or test for ACL completeness

Add an automated check that verifies all commands registered in `lib.rs` have corresponding entries in `build.rs` and `capabilities/main.json`. This could be:

- A unit test that parses all three files and asserts the sets match.
- A pre-commit script that diffs the three command lists.

This would catch the ACL registration problem at build/commit time rather than at runtime.
