#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""
Classify unsupported cases into test cases.

Reads unsupported-cases.json, prompts the user to classify each case,
and moves classified cases into test-cases.json.
"""

import json
import pathlib
import subprocess
import sys


SCRIPT_DIR = pathlib.Path(__file__).parent
sys.path.insert(0, str(SCRIPT_DIR))
from redact import redact_entry
UNSUPPORTED_CASES_FILE = pathlib.Path.home() / ".claude" / "cc-judge-unsupported-cases.json"
TEST_CASES_FILE = SCRIPT_DIR / "test-cases.json"

VALID_CHOICES = {
    "a": "approved",
    "dec": "declined",
    "def": "deferred",
    "del": "delete",
    "s": "skip",
}


def read_json(path: pathlib.Path) -> list:
    try:
        with open(path, "r") as f:
            return json.load(f)
    except FileNotFoundError:
        return []


def write_json(path: pathlib.Path, data: list):
    with open(path, "w") as f:
        json.dump(data, f, indent=4)


EXIT_CODE_TO_RESULT = {
    0: "approved",
    2: "declined",
}


def get_actual_result(case: dict) -> str:
    result = subprocess.run(
        [str(SCRIPT_DIR / "main.py")],
        input=json.dumps(case),
        capture_output=True,
        text=True,
    )
    return EXIT_CODE_TO_RESULT.get(result.returncode, "deferred")


def prompt_choice() -> str:
    while True:
        choice = input("  (a)pproved / (dec)lined / (def)erred / (del)ete / (s)kip: ").strip().lower()
        if choice in VALID_CHOICES:
            return choice
        print(f"  Invalid choice: {choice!r}")


def main():
    unsupported = read_json(UNSUPPORTED_CASES_FILE)

    if not unsupported:
        print("No unsupported cases to classify.")
        sys.exit(0)

    test_cases = read_json(TEST_CASES_FILE)

    total = len(unsupported)
    print(f"Found {total} unsupported case(s) to classify.\n")

    for i, case in enumerate(list(unsupported)):
        tool_name = case.get("tool_name", "<unknown>")
        command = case.get("tool_input", {}).get("command", "<no command>")
        cwd = case.get("cwd", "<no cwd>")
        actual_result = get_actual_result(case)

        print(f"[{i + 1}/{total}] tool: {tool_name}")
        print(f"  command: {command}")
        print(f"  cwd: {cwd}")
        print(f"  actual result: {actual_result}")

        if actual_result == "approved":
            unsupported.remove(case)
            write_json(UNSUPPORTED_CASES_FILE, unsupported)
            print(f"  -> auto-approved by main.py, removed\n\n")
            continue

        choice = prompt_choice()
        result = VALID_CHOICES[choice]

        if result == "skip":
            msg = "skipped"
        elif result == "delete":
            unsupported.remove(case)
            write_json(UNSUPPORTED_CASES_FILE, unsupported)
            msg = "deleted"
        else:
            test_cases.append([redact_entry(case), result])
            unsupported.remove(case)
            write_json(TEST_CASES_FILE, test_cases)
            write_json(UNSUPPORTED_CASES_FILE, unsupported)
            msg = f"classified as {result}"
        print(f"  -> {msg}\n\n\n------------\n\n")


if __name__ == "__main__":
    main()
