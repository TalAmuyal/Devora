#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""
Claude Code permission manager hook
===================================

Using Claude Code's `PermissionRequest` hook.
Applies allow/deny rules based on a custom logic.

Exit codes (Claude Code `PermissionRequest` hook protocol):
- Exit 0: Success — Claude Code parses stdout JSON for the allow/deny decision
- Exit 2: Blocking error — denies permission, stderr shown to Claude
- Any other code (including 1): Non-blocking error — hook treated as failed, execution continues normally

"""
import argparse
import json
import pathlib
import shlex
import sys
import typing


UNSUPPORTED_CASES_FILE = pathlib.Path.home() / ".claude" / "cc-judge-unsupported-cases.json"


Command = list[str]
BooleanDetector: typing.TypeAlias = typing.Callable[[Command, str], bool]
DeclineDetector: typing.TypeAlias = typing.Callable[[Command, str], str | None]


ASK_USER = {
    "chmod",
}

MSG_USE_MISE_TASK = "Please use a mise task or `mise exec -- ...` instead (create a generic task if needed)."

DECLINED_COMMANDS = (
    (
        ["eval"],
        "Command 'eval' is not allowed.",
    ),
    (
        ["go"],
        MSG_USE_MISE_TASK,
    ),
    (
        ["npm"],
        MSG_USE_MISE_TASK,
    ),
    (
        ["npx"],
        MSG_USE_MISE_TASK,
    ),
)

ALLOWED_COMMANDS = {
    "for",
    "cat",
    "echo",
    "mise",
    "test",
    "awk",
    "find",
    "basename",
    "ls",
    "tree",
    "tr",
    "which",
    "make",
    "cp",
    "mkdir",
    "wc",
    "true",
    "head",
    "tail",
    "stat",
    "tee",
    "sort",
    "uniq",
    "grep",
    "hash",
    "fetch-gh-file",
    "gaac",
    "python",
    "python3",
    ".venv/bin/python",
    "./.venv/bin/python",
    "submit-pr",
}

DERISKING_FLAGS = {
    "--help",
    "--version",
}

RISKY_FIND_FLAGS = {
    "-delete",
    "-exec",
    "-execdir",
    "-ok",
    "-okdir",
}


def words_decliner(
    command: Command,
    cwd: str | None,
) -> str | None:
    for words, msg in DECLINED_COMMANDS:
        if command[0:len(words)] == words:
            return msg
    return None


def validate_sed(
    command: Command,
    cwd: str | None,
) -> bool:
    """
    Tries to assert that a sed command is safe.

    For example, the following is OK:
    - ['sed', '-n', '500,1000p', '/tmp/mise_lint_output.txt']
    """
    command = command.copy()  # Don't mutate the original
    if not pop_if_exists(command, "sed"):
        return False

    if "-i" in command:
        return False  # Can't auto-allow in-place editing

    command.remove("-n")

    while command:
        token = command.pop(0)
        if token.endswith("p") and token[:-1].replace(",", "").isdigit():
            pass  # E.g. "500,1000p"
        elif token.startswith("/tmp/"):
            pass  # /tmp is not a problem
        elif token == cwd or token.startswith(cwd + "/"):
            pass  # CWD is not a problem
        else:
            return False

    return True


def pop_if_exists(lst: list[str], value: str) -> bool:
    if value in lst:
        lst.remove(value)
        return True
    return False



class Detectors:
    DECLINED: list[DeclineDetector] = [
        words_decliner,
    ]
    KNOWN_TO_NOT_HANDLE: list[BooleanDetector] = [
        lambda cmd, _: cmd[0] in ASK_USER,
        lambda cmd, _: cmd[0].startswith("./"),
        lambda cmd, _: cmd[0] == "find" and RISKY_FIND_FLAGS & set(cmd),
    ]
    APPROVED: list[BooleanDetector] = [
        lambda cmd, _: cmd[0] in ALLOWED_COMMANDS,
        lambda cmd, _: any(
            cmd[0:len(allowed)] == allowed
            for allowed in [
                ["git", "diff"],
                ["git", "grep"],
                ["git", "show"],
                ["git", "log"],
                ["git", "ls-files"],
                ["git", "status"],
                ["git", "ls-tree"],
                ["git", "rm"],
                ["git" ,"mv"],
                ["git", "remote", "get-url"],
                ["git", "rev-parse"],
                ["go", "doc"],
                ["helm", "dependency"],
                ["helm", "version"],
                ["mise", "help"],
                ["mise", "ls"],
                ["mise", "usage"],
                [".venv/bin/ruff", "check"],
                [".venv/bin/ruff", "format"],
                ["jar", "tf"],
            ]
        ),
        lambda cmd, _: cmd[0] == "find" and not (RISKY_FIND_FLAGS & set(cmd)),
        lambda cmd, _: bool(frozenset(cmd[1:]) & DERISKING_FLAGS),
        validate_sed,
        lambda cmd, _: cmd == ["done"],  # Allow "done" to close loops like "for x in ...; do ...; done"
    ]


IRRELEVANT_PREFIXES = {
    # Redirects to another command
    "xargs",
    "do",
    #
    # A harmless env-vars
    "MISE_VERBOSE=1",
    "MISE_PREPARE=false",
    'MISE_PREPARE="false"',
    "MISE_PREPARE='false'",
    "MISE_PREPARE_SKIP=1",
    "MISE_PREPARE_SKIP=0",
    "PYTHONPATH=.",
}


def _debug_mismatch(expected: str | None, actual: str, reason: str) -> None:
    if expected is None or expected == actual:
        return
    print(f"MISMATCH: expected {expected}, got {actual}: {reason}", file=sys.stderr)


def remove_irrelevant_parts(command: Command) -> None:
    while command and command[0] in IRRELEVANT_PREFIXES:
        command.pop(0)


def main(args: dict, expected: str | None) -> None:
    if args.get("tool_name", "Bash") != "Bash":
        _debug_mismatch(expected, "deferred", "not a Bash tool")
        direct_to_user_and_exit()

    tool_input: dict = args["tool_input"]
    input_command: str = tool_input.get("command")
    if not input_command:
        _debug_mismatch(expected, "deferred", "no command in tool_input")
        direct_to_user_and_exit()

    session_cwd: str | None = args.get("cwd")
    safe_cwd = session_cwd or "!@#$%^&*()_+-this-is not-a-valid-path"

    commands = parse_command(input_command)
    if expected is not None:
        print("Parsed:", commands, flush=True, file=sys.stderr)

    for cmd in commands:
        remove_irrelevant_parts(cmd)

        if (
            cmd[0] == "git"
            and "-C" in cmd
            and ((c_index := cmd.index("-C")) + 1 < len(cmd))
            and is_safe_path(
                tested_path=cmd[c_index + 1],
                safe_cwd=safe_cwd,
            )
        ):
            cmd.pop(c_index)  # Remove the "-C"
            cmd.pop(c_index)  # Remove the path as well

        for check in Detectors.KNOWN_TO_NOT_HANDLE:
            if check(cmd, safe_cwd):
                _debug_mismatch(expected, "deferred", f"known to not handle: {cmd}")
                direct_to_user_and_exit()

        if (
            len(cmd) == 2
            and cmd[0] == "cd"
            and is_safe_path(
                tested_path=cmd[1],
                safe_cwd=safe_cwd,
            )
        ):
            continue  # Allow "cd" into the session's current working directory

        for check in Detectors.DECLINED:
            if msg := check(cmd, safe_cwd):
                _debug_mismatch(expected, "declined", f"declined command: {cmd[0]}")
                deny_and_exit(msg)

        if any(check(cmd, safe_cwd) for check in Detectors.APPROVED):
            continue  # Check the next command
        else:
            if expected is None:
                save_unsupported_case(args)
            _debug_mismatch(expected, "deferred", f"no approved detector matched: {cmd}")
            direct_to_user_and_exit()

    _debug_mismatch(expected, "approved", "all commands approved")
    allow_and_exit(input_command)


def parse_command(command: str) -> list[list[str]]:
    lexer = shlex.shlex(command, posix=True, punctuation_chars=True)

    commands: list[list[str]] = []
    current_command: list[str] | None = None

    tick_in_progress = False
    parens_in_progress = 0

    for token in lexer:
        if token.startswith("#"):
            break  # Ignore comments

        if current_command is None:
            current_command = []
            commands.append(current_command)

        if token in {"&&", "||", "|", ";"}:
            current_command = None
        elif token.startswith("$("):
            if token.endswith(")"):
                current_command = None
                commands.append([token.removeprefix("$(").removesuffix(")")])
            else:
                parens_in_progress += 1
                current_command = [token.removeprefix("$(")]
                commands.append(current_command)
        elif token.endswith(")"):
            parens_in_progress -= 1
            if parens_in_progress < 0:
                direct_to_user_and_exit()
            current_command = None
        elif token == "`":
            if tick_in_progress:
                current_command = None
                tick_in_progress = False
            else:
                tick_in_progress = True
                current_command = None
        elif token.startswith("`"):
            if token.endswith("`"):
                current_command = None
                commands.append([token.removeprefix("`").removesuffix("`")])
            elif tick_in_progress:
                direct_to_user_and_exit()
            else:
                tick_in_progress = True
                current_command = [token.removeprefix("`")]
                commands.append(current_command)
        elif token.endswith("`"):
            if not tick_in_progress:
                direct_to_user_and_exit()
            tick_in_progress = False
            current_command.append(token.removesuffix("`"))
            current_command = None
        else:
            current_command.append(token)

    if parens_in_progress != 0 or tick_in_progress:
        direct_to_user_and_exit()

    return commands


def is_safe_path(
    tested_path: str,
    safe_cwd: str,
) -> bool:
    return tested_path == safe_cwd or tested_path.startswith(safe_cwd + "/")


def save_unsupported_case(args: dict):
    if UNSUPPORTED_CASES_FILE.exists():
        with open(UNSUPPORTED_CASES_FILE, "r") as f:
            cases = json.load(f)
    else:
        cases = []

    if args not in cases:
        cases.append(args)
        with open(UNSUPPORTED_CASES_FILE, "w") as f:
            json.dump(cases, f, indent=4)


def allow_and_exit(
    updated_command: str,
) -> None:
    result = {
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": {
                "behavior": "allow",
                "updatedInput": {
                    "command": updated_command,
                }
            }
        }
    }

    print(json.dumps(result))
    sys.exit(0)


def deny_and_exit(error_message: str):
    print(error_message, file=sys.stderr)
    sys.exit(2)


def direct_to_user_and_exit():
    sys.exit(1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected", choices=["approved", "declined", "deferred"])
    cli_args = parser.parse_args()

    raw_input: str = sys.stdin.read().strip()
    parsed_input: dict = json.loads(raw_input)

    main(parsed_input, expected=cli_args.expected)
