# Command Validator

I really dislike having to manually go through so many permission prompts.
This projects extends the ability to allow/denies based on more complex rules.

It runs after the permissions block in your settings, so allowing here and denying there will still deny.

git -C <some long path> <command I already allowed> ...

## Setup

- Install `uv`
- Run `chmod +x ./main.py`
- Add to `~/.claude/settings.json`:

```json
    "PermissionRequest": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/path/to/main.py"
          }
        ]
      }
    ],
```

## Flow

The script decides whether to allow or deny a permission request based on the command and its arguments.
When it encounters a situation that it doesn't know how to handle, it saves it as an example case and defer the decision to the user, allowing them to review and decide on it later.

## Unhandled tool types

Judge currently only reasons about `Bash` permission requests. When it sees a request for any other tool type, it defers to the user and appends the raw request to `~/.claude/cc-judge-unhandled-requests.json`. This file is the source for adding support for new tool types.

## Testing

An end-to-end test is done by running the script with known inputs and matching that to an expected result.
The test cases are stored in `./test-cases.json`.

## Redaction

`redact.py` sanitizes `test-cases.json` before it enters the repo, removing PII and sensitive content.
It redacts home directory paths, path segments (hashed to `project-XXXXX`), git hashes, commit messages/PR titles, timestamps, and `description` fields.

```
./redact.py           # Preview redacted output on stdout
./redact.py --apply   # Redact test-cases.json in place
./redact.py --audit   # Verify no suspicious content remains
```
