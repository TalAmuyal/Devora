#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""
Redact PII from test case entries.

Replaces home directory paths, session IDs, and agent IDs with consistent
placeholders. Preserves structural relationships between fields (e.g., if cwd
appears in tool_input.command, both get the same placeholder).

Usage:
    ./redact.py           # Preview redacted test-cases.json on stdout
    ./redact.py --apply   # Redact test-cases.json in place
"""

import json
import os
import pathlib
import re
import sys


SCRIPT_DIR = pathlib.Path(__file__).parent
TEST_CASES_FILE = SCRIPT_DIR / "test-cases.json"

REDACTED_HOME = "/home/user"
REDACTED_ENCODED_HOME = "home-user"

REMOVED_FIELDS = {
    "session_id",
    "transcript_path",
    "permission_mode",
    "hook_event_name",
    "tool_name",
    "permission_suggestions",
    "agent_id",
    "agent_type",
}


def detect_home_dir(entry: dict) -> str | None:
    """Detect the home directory path from the entry's path fields."""
    for field in ("cwd", "session_cwd"):
        path = entry.get(field)
        if path:
            match = re.match(r"(/(?:Users|home)/[^/]+)", path)
            if match:
                return match.group(1)
    return None


def encode_as_claude_project_path(path: str) -> str:
    """Encode a path the way Claude Code does for project directory names.

    /Users/tal_amuyal → Users-tal-amuyal
    """
    return path.lstrip("/").replace("/", "-").replace("_", "-").replace(".", "")


def build_replacements(entry: dict) -> dict[str, str]:
    """Build a mapping of real values to redacted placeholders."""
    replacements = {}

    # Real home directory from the OS (handles new cases and encoded paths)
    real_home = os.path.expanduser("~")
    replacements[real_home] = REDACTED_HOME
    replacements[encode_as_claude_project_path(real_home)] = REDACTED_ENCODED_HOME

    # Entry's home directory (might differ if already partially anonymized)
    entry_home = detect_home_dir(entry)
    if entry_home and entry_home != real_home:
        replacements[entry_home] = REDACTED_HOME
        encoded = encode_as_claude_project_path(entry_home)
        if encoded not in replacements:
            replacements[encoded] = REDACTED_ENCODED_HOME

    return replacements


def apply_replacements(obj, replacements: dict[str, str]):
    """Recursively apply string replacements, longest match first."""
    if isinstance(obj, str):
        for old in sorted(replacements, key=len, reverse=True):
            obj = obj.replace(old, replacements[old])
        return obj
    elif isinstance(obj, dict):
        return {k: apply_replacements(v, replacements) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [apply_replacements(item, replacements) for item in obj]
    return obj


def redact_entry(entry: dict) -> dict:
    """Redact PII from a single test case entry."""
    replacements = build_replacements(entry)
    redacted = apply_replacements(entry, replacements)
    for field in REMOVED_FIELDS:
        redacted.pop(field, None)
    return redacted


def redact_test_cases(test_cases: list) -> list:
    """Redact all test cases in the list."""
    return [[redact_entry(case), result] for case, result in test_cases]


def main():
    apply = "--apply" in sys.argv

    with open(TEST_CASES_FILE, "r") as f:
        test_cases = json.load(f)

    redacted = redact_test_cases(test_cases)

    if apply:
        with open(TEST_CASES_FILE, "w") as f:
            json.dump(redacted, f, indent=4)
        print(f"Redacted {len(test_cases)} test cases in {TEST_CASES_FILE}")
    else:
        print(json.dumps(redacted, indent=4))


if __name__ == "__main__":
    main()
