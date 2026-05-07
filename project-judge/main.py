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
import fcntl
import json
import os
import pathlib
import shlex
import sys
import time
import typing
from datetime import datetime, timezone


UNSUPPORTED_CASES_FILE = pathlib.Path.home() / ".claude" / "cc-judge-unsupported-cases.json"
UNHANDLED_REQUESTS_FILE = pathlib.Path.home() / ".claude" / "cc-judge-unhandled-requests.json"
AUDIT_LOG_FILE = pathlib.Path.home() / ".claude" / "cc-judge-audit.jsonl"
ABSTAIN_TOOL_NAMES: set[str] = {"AskUserQuestion", "Edit"}


Command = list[str]
BooleanDetector: typing.TypeAlias = typing.Callable[[Command, str], bool]
DeclineDetector: typing.TypeAlias = typing.Callable[[Command, str], str | None]
LabeledBooleanDetector: typing.TypeAlias = tuple[str, BooleanDetector]
LabeledDeclineDetector: typing.TypeAlias = tuple[str, DeclineDetector]

_trail: list[str] = []
_start_ns: int = 0
_input_args: dict = {}
_test_mode: bool = False


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
        ["node"],
        MSG_USE_MISE_TASK,
    ),
    (
        ["bun"],
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
    (
        ["poetry"],
        MSG_USE_MISE_TASK,
    ),
    (
        ["shellcheck"],
        MSG_USE_MISE_TASK,
    ),
    (
        ["helm"],
        MSG_USE_MISE_TASK,
    ),
)

ALLOWED_COMMANDS = {
    "for",
    "done",  # Allow "done" to close loops like "for x in ...; do ...; done"
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
    "printf",
    "which",
    "make",
    "cp",
    "mkdir",
    "wc",
    "nm",
    "otool",
    "true",
    "head",
    "tail",
    "stat",
    "tee",
    "sort",
    "uniq",
    "grep",
    "hash",
    "gaac",
    "python",
    "python3",
    ".venv/bin/python",
    "./.venv/bin/python",
    "submit-pr",
    "man",
    "ruff",
}

ALLOWED_EXACT_MATCHES = {
    ("git", "branch"),
    ("git", "branch", "-r"),
    ("git", "branch", "-a"),
    ("git", "branch", "--all"),
    ("git", "branch", "--show-current"),
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
    DECLINED: list[LabeledDeclineDetector] = [
        ("words_decliner", words_decliner),
    ]
    KNOWN_TO_NOT_HANDLE: list[LabeledBooleanDetector] = [
        ("dotslash_command", lambda cmd, _: cmd[0].startswith("./")),
        ("risky_find_flags", lambda cmd, _: cmd[0] == "find" and RISKY_FIND_FLAGS & set(cmd)),
        ("bash_not_syntax_check", lambda cmd, _: cmd[0] == "bash" and cmd[1] != "-n"),
        ("dangerous_command", lambda cmd, _: any(
            cmd[0:len(cmd_start)] == cmd_start
            for cmd_start in [
                ["chmod"],
                ["rm"],
                ["git", "add"],
                ["git", "checkout"],
                ["git", "reset"],
            ]
        )),
    ]
    APPROVED: list[LabeledBooleanDetector] = [
        ("command_allowlist", lambda cmd, _: cmd[0] in ALLOWED_COMMANDS),
        ("source_venv_cwd", lambda cmd, cwd: (
            len(cmd) == 2
            and cmd[0] == "source"
            and cmd[1].startswith(cwd + "/")
            and cmd[1].endswith("/.venv/bin/activate")
        )),
        ("safe_prefix", lambda cmd, _: any(
            cmd[0:len(allowed)] == allowed
            for allowed in [
                ["command", "-v"],
                ["bash", "-n"],
                ["source", ".venv/bin/activate"],
                ["git", "diff"],
                ["git", "grep"],
                ["git", "show"],
                ["git", "log"],
                ["git", "tag", "-l"],
                ["git", "tag", "--list"],
                ["git", "config", "--list"],
                ["git", "notes", "list"],
                ["git", "ls-files"],
                ["git", "status"],
                ["git", "ls-tree"],
                ["git", "rm"],
                ["git", "mv"],
                ["git", "remote", "get-url"],
                ["git", "remote", "-v"],
                ["git", "rev-parse"],
                ["git", "check-ignore"],
                ["git", "help"],
                ["go", "doc"],
                ["helm", "dependency"],
                ["helm", "version"],
                ["mise", "help"],
                ["mise", "ls"],
                ["mise", "usage"],
                [".venv/bin/ruff", "check"],
                [".venv/bin/ruff", "format"],
                ["jar", "tf"],
                ["brew", "list"],
            ]
        )),
        ("safe_find", lambda cmd, _: cmd[0] == "find" and not (RISKY_FIND_FLAGS & set(cmd))),
        ("derisking_flags", lambda cmd, _: bool(frozenset(cmd[1:]) & DERISKING_FLAGS)),
        ("validate_sed", validate_sed),
        ("exact_match", lambda cmd, _: tuple(cmd) in ALLOWED_EXACT_MATCHES),
    ]


_DANGEROUS_ENV_VARS = {
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "BASH_ENV",
    "ENV",
    "NODE_OPTIONS",
    "PYTHONSTARTUP",
    "GIT_EXEC_PATH",
    "GIT_SSH_COMMAND",
    "GIT_TEMPLATE_DIR",
    "EDITOR",
    "VISUAL",
    "PAGER",
}

IRRELEVANT_PREFIXES = {
    # Redirects to another command
    "xargs",
    "do",
    "time",
    "watch",
}


def _debug_mismatch(expected: str | None, actual: str, reason: str) -> None:
    if expected is None or expected == actual:
        return
    print(f"MISMATCH: expected {expected}, got {actual}: {reason}", file=sys.stderr)


def _is_env_var_prefix(token: str) -> bool:
    if "=" not in token:
        return False
    name = token.split("=", 1)[0]
    return name.isidentifier() and name not in _DANGEROUS_ENV_VARS


def remove_irrelevant_parts(command: Command) -> None:
    if len(command) > 2 and command[0] == "timeout":
        arg = command[1]
        if arg.endswith("s"):
            arg = arg[:-1]

        is_number = False
        try:
            is_number = float(arg) >= 0
        except ValueError:
            pass

        if is_number:
            command.pop(0)  # Remove "timeout"
            command.pop(0)  # Remove the duration
            remove_irrelevant_parts(command)
    elif command and command[0] in IRRELEVANT_PREFIXES:
        command.pop(0)
        remove_irrelevant_parts(command)
    elif command and _is_env_var_prefix(command[0]):
        command.pop(0)
        remove_irrelevant_parts(command)


def main(args: dict, expected: str | None) -> None:
    global _trail, _start_ns, _input_args
    _trail = []
    _start_ns = time.monotonic_ns()
    _input_args = args

    tool_name: str = args.get("tool_name")
    tool_input: dict = args.get("tool_input", {})

    if tool_name in ABSTAIN_TOOL_NAMES:
        abstain_and_exit()

    if tool_name == "WebFetch":
        _trail.append("tool_type=WebFetch")
        url = tool_input.get("url", "")
        if url.startswith("file://"):
            _trail.append("scheme=file")
            deny_and_exit(
                "`WebFetch` access to local files is not allowed, use the `Read` tool instead.",
                "webfetch.file_url_blocked",
            )
        elif url.startswith("http://") or url.startswith("https://"):
            _trail.append("scheme=http")
            direct_to_user_and_exit("webfetch.http_url")
        else:
            _trail.append("scheme=unknown")
            save_unhandled_request(args)
            direct_to_user_and_exit("webfetch.unknown_scheme")
    elif tool_name == "Read":
        _trail.append("tool_type=Read")
        file_path = tool_input.get("file_path", "")
        crit_dir_path = pathlib.Path.home() / ".crit"
        read_allowed_paths = {
            str(crit_dir_path / "plans") + "/",
            str(crit_dir_path / "reviews") + "/",
        }
        if any(file_path.startswith(allowed) for allowed in read_allowed_paths):
            _trail.append("crit_path=True")
            allow_and_exit(tool_input, "read.crit_path")
        else:
            _trail.append("crit_path=False")
            save_unhandled_request(args)
            direct_to_user_and_exit("read.unknown_path")
    elif tool_name != "Bash":
        _trail.append(f"tool_type={tool_name}")
        if expected is None:
            save_unhandled_request(args)
        _debug_mismatch(expected, "deferred", "not a Bash tool")
        direct_to_user_and_exit("tool.unknown_type")

    _trail.append("tool_type=Bash")
    input_command: str = tool_input.get("command")
    if not input_command:
        _trail.append("command=empty")
        _debug_mismatch(expected, "deferred", "no command in tool_input")
        direct_to_user_and_exit("bash.no_command")

    session_cwd: str | None = args.get("cwd")
    safe_cwd = session_cwd or "!@#$%^&*()_+-this-is not-a-valid-path"

    commands = parse_command(input_command)
    if expected is not None:
        print("Parsed:", commands, flush=True, file=sys.stderr)

    for i, cmd in enumerate(commands):
        remove_irrelevant_parts(cmd)

        if not cmd:
            continue

        if (
            cmd[0] == "git"
            and "-C" in cmd
            and ((c_index := cmd.index("-C")) + 1 < len(cmd))
            and is_safe_path(
                tested_path=cmd[c_index + 1],
                safe_cwd=safe_cwd,
            )
        ):
            _trail.append(f"cmd[{i}]:strip_git_C")
            cmd.pop(c_index)  # Remove the "-C"
            cmd.pop(c_index)  # Remove the path as well

        for label, check in Detectors.KNOWN_TO_NOT_HANDLE:
            if check(cmd, safe_cwd):
                _trail.append(f"cmd[{i}]:known_unhandled:{label}=MATCH")
                _debug_mismatch(expected, "deferred", f"known to not handle: {cmd}")
                direct_to_user_and_exit(
                    "bash.known_unhandled",
                    details={"command": cmd, "detector": label},
                )

        if (
            len(cmd) == 2
            and cmd[0] == "cd"
            and is_safe_path(
                tested_path=cmd[1],
                safe_cwd=safe_cwd,
            )
        ):
            _trail.append(f"cmd[{i}]:safe_cd")
            continue

        for label, check in Detectors.DECLINED:
            if msg := check(cmd, safe_cwd):
                _trail.append(f"cmd[{i}]:declined:{label}=MATCH")
                _debug_mismatch(expected, "declined", f"declined command: {cmd[0]}")
                deny_and_exit(
                    msg,
                    "bash.declined_command",
                    details={"command": cmd, "detector": label},
                )

        approved = False
        for label, check in Detectors.APPROVED:
            if check(cmd, safe_cwd):
                _trail.append(f"cmd[{i}]:approved:{label}=MATCH")
                approved = True
                break
        if approved:
            continue
        else:
            _trail.append(f"cmd[{i}]:no_approved_detector")
            if expected is None:
                save_unsupported_case(args)
            _debug_mismatch(expected, "deferred", f"no approved detector matched: {cmd}")
            direct_to_user_and_exit(
                "bash.no_approved_detector",
                details={"command": cmd},
            )

    _debug_mismatch(expected, "approved", "all commands approved")
    allow_and_exit(input_command, "bash.all_commands_approved")


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
                _trail.append("parse_unsafe:unbalanced_close_paren")
                direct_to_user_and_exit("bash.parse_unsafe", details={"issue": "unbalanced_close_paren"})
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
                _trail.append("parse_unsafe:nested_backtick")
                direct_to_user_and_exit("bash.parse_unsafe", details={"issue": "nested_backtick"})
            else:
                tick_in_progress = True
                current_command = [token.removeprefix("`")]
                commands.append(current_command)
        elif token.endswith("`"):
            if not tick_in_progress:
                _trail.append("parse_unsafe:unexpected_backtick_close")
                direct_to_user_and_exit("bash.parse_unsafe", details={"issue": "unexpected_backtick_close"})
            tick_in_progress = False
            current_command.append(token.removesuffix("`"))
            current_command = None
        else:
            current_command.append(token)

    if parens_in_progress != 0 or tick_in_progress:
        _trail.append("parse_unsafe:unclosed_subexpression")
        direct_to_user_and_exit("bash.parse_unsafe", details={"issue": "unclosed_subexpression"})

    return commands


def is_safe_path(
    tested_path: str,
    safe_cwd: str,
) -> bool:
    return tested_path == safe_cwd or tested_path.startswith(safe_cwd + "/")


def _locked_json_append(file_path: pathlib.Path, entry: dict) -> None:
    try:
        file_path.touch(exist_ok=True)
        with open(file_path, "r+") as f:
            fcntl.flock(f.fileno(), fcntl.LOCK_EX)
            content = f.read()
            entries = json.loads(content) if content.strip() else []
            if entry not in entries:
                entries.append(entry)
                f.seek(0)
                f.truncate()
                json.dump(entries, f, indent=4)
    except Exception:
        pass


def save_unsupported_case(args: dict):
    _locked_json_append(UNSUPPORTED_CASES_FILE, args)


def save_unhandled_request(args: dict):
    _locked_json_append(UNHANDLED_REQUESTS_FILE, args)


def _write_audit_entry(decision: str, reason: str, details: dict | None = None) -> None:
    if _test_mode:
        return
    try:
        entry = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "pid": os.getpid(),
            "session_id": _input_args.get("session_id"),
            "agent_id": _input_args.get("agent_id"),
            "tool_name": _input_args.get("tool_name"),
            "tool_input": _input_args.get("tool_input"),
            "cwd": _input_args.get("cwd"),
            "decision": decision,
            "reason": reason,
            "trail": _trail,
            "duration_us": (time.monotonic_ns() - _start_ns) // 1000,
        }
        if details:
            entry["details"] = details
        line = json.dumps(entry, separators=(",", ":")) + "\n"
        fd = os.open(str(AUDIT_LOG_FILE), os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
        try:
            os.write(fd, line.encode())
        finally:
            os.close(fd)
    except Exception:
        pass


def allow_and_exit(
    updated_tool_input: dict,
    reason: str,
    details: dict | None = None,
) -> None:
    _write_audit_entry("allow", reason, details)
    result = {
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": {
                "behavior": "allow",
                "updatedInput": updated_tool_input,
            }
        }
    }
    print(json.dumps(result))
    sys.exit(0)


def deny_and_exit(error_message: str, reason: str, details: dict | None = None):
    _write_audit_entry("deny", reason, details)
    print(error_message, file=sys.stderr)
    sys.exit(2)


def direct_to_user_and_exit(reason: str, details: dict | None = None):
    _write_audit_entry("defer", reason, details)
    sys.exit(1)


def abstain_and_exit():
    sys.exit(0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected", choices=["approved", "declined", "deferred"])
    cli_args = parser.parse_args()

    _test_mode = cli_args.expected is not None
    _start_ns = time.monotonic_ns()

    raw_input: str = sys.stdin.read().strip()

    try:
        parsed_input: dict = json.loads(raw_input)
    except json.JSONDecodeError as e:
        _write_audit_entry(
            "error",
            "invalid_json_input",
            details={"error": str(e), "raw_input_preview": raw_input[:200]},
        )
        sys.exit(1)

    _input_args = parsed_input

    try:
        main(parsed_input, expected=cli_args.expected)
    except Exception as e:
        import traceback
        _write_audit_entry(
            "error",
            "uncaught_exception",
            details={
                "error": str(e),
                "traceback": traceback.format_exc(),
            },
        )
        sys.exit(1)
