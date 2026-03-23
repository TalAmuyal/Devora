# Task Package Spec

Package: `internal/task`

## Purpose

Create task metadata files for workspaces. A task represents a unit of work associated with a workspace.

## Task JSON Format

The task file (`task.json`) is written to the workspace root:

```json
{
    "uid": "550e8400-e29b-41d4-a716-446655440000",
    "title": "implement auth flow",
    "started_at": "2026-03-13"
}
```

Fields:
- `uid` - UUID v4, generated at creation time
- `title` - Human-readable task name, provided by the user
- `started_at` - ISO 8601 date (YYYY-MM-DD) of the creation date

The file is written with 4-space indentation for readability.

## Functions

### Create

```go
func Create(title string, workspaceTaskPath string) error
```

Creates a new task:
1. Generate a UUID v4.
2. Get today's date in `YYYY-MM-DD` format.
3. Build the task JSON with `uid`, `title`, and `started_at`.
4. Write to `workspaceTaskPath` with 4-space indented JSON and mode `0644`.

The `workspaceTaskPath` is the absolute path to the `task.json` file (provided by the caller, typically from `workspace.GetWorkspaceTaskPath()`).

### UpdateTitle

```go
func UpdateTitle(newTitle string, workspaceTaskPath string) error
```

Updates the title of an existing task:
1. Read the task file from `workspaceTaskPath`.
2. Parse the JSON into the task struct.
3. Replace the `title` field with `newTitle`.
4. Write the updated JSON back with 4-space indentation and mode `0644`.

All other fields (`uid`, `started_at`) are preserved. Returns an error if the file does not exist, cannot be parsed, or cannot be written.

## Dependencies

No external dependencies (stdlib only).

## Testing

- Test that `Create` writes valid JSON with the expected fields.
- Test that the UUID is a valid v4 UUID format.
- Test that `started_at` is today's date.
- Test that the file is properly indented (4 spaces).
- Test that `Create` generates unique UIDs across multiple calls.
- Use a temp directory for the workspace task path.
