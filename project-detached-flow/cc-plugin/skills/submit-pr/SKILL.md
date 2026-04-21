---
name: submit-pr
description: Submit a pull request (Devora)
user-invocable: true
disable-model-invocation: false
argument-hint: Optional further instructions
---
Please follow the instructions below to submit a PR (pull request) with a message and a description.

$ARGUMENTS

## Workflow

1. Run `submit-pr -q -m "message" -d "description"`
2. Provide back the PR link generated, preferably in a dedicated line and without any additional text around it, so that I can easily identify and click on it
3. Wait for further instructions

`submit-pr ...` creates:
- A git commit with the provided message
- A branch named based on the message (formatted variant of the message)
- A PR with the message as the title and the description as the body/description

## Message Guidelines

Write simple, readable messages describing what was done:

```bash
# Good messages
submit-pr -q -m "Add retry logic for API calls" ...
submit-pr -q -m "Fix calculation error in fraud score" ...
submit-pr -q -m "Update validation rules for INR transactions" ...

# Bad messages
submit-pr -q -m "fix" ... # Too vague
submit-pr -q -m "ADD-RETRY-LOGIC-FOR-API" ...  # Bad formatting, should be written naturally
```

`submit-pr` automatically formats your message appropriately for each context (branch names use dashes, PR titles preserve spaces, etc.).

## Description Guidelines

Good:
- Formatted as a multi-line Markdown string
- Concise
- Summarizes the "what" and the "why" of the change, with any other important details for reviewers/maintainers

Bad:
- A single line of text
- Includes testing instructions or a test plan

## Complete Example

```bash
submit-pr -q -m "Add retry logic for API calls" -d '## What

- Make the service more resilient to transient network issues and reduce failure rates for API calls
- Implement exponential backoff with jitter for retries

## Why

- Had multiple incidents last month where API calls failed due to temporary network issues, causing user-facing errors and increased support tickets
- This change will improve reliability and user experience by automatically retrying failed API calls with a robust strategy, without requiring manual intervention or support tickets for transient issues'
```

## Common Issues

### Not on Detached HEAD

If you're already on a branch, it probably means that `submit-pr` has already been executed and further branch-related git operations should be avoided.
Instead, let the user handle follow-up commits and similar operations on their own.

### Uncommitted Changes

Ensure all changes you want included are saved.
`submit-pr` adds and commits all tracked and untracked files (except those in `.gitignore`).

### Authentication Issues

If `submit-pr` fails with authentication errors, let the user know.
