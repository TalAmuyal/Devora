#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""
Run test cases against main.py and report results.

Reads test-cases.json, pipes each input to main.py via stdin,
and compares the exit code to the expected result.
"""

import json
import pathlib
import subprocess
import sys


SCRIPT_DIR = pathlib.Path(__file__).parent
TEST_CASES_FILE = SCRIPT_DIR / "test-cases.json"
MAIN_SCRIPT = SCRIPT_DIR / "main.py"

EXIT_CODE_TO_RESULT = {
    0: "approved",
    2: "declined",
}


def map_exit_code(exit_code: int) -> str:
    return EXIT_CODE_TO_RESULT.get(exit_code, "deferred")


def run_test(input_data: dict, expected: str) -> tuple[str, str, str]:
    result = subprocess.run(
        [sys.executable, str(MAIN_SCRIPT), "--expected", expected],
        input=json.dumps(input_data),
        capture_output=True,
        text=True,
    )
    return map_exit_code(result.returncode), result.stdout, result.stderr


def main():
    with open(TEST_CASES_FILE, "r") as f:
        test_cases = json.load(f)

    if not test_cases:
        print("No test cases found.")
        sys.exit(0)

    passed = 0
    failed = 0

    for case_input, expected in test_cases:
        actual, stdout, stderr = run_test(case_input, expected)
        if actual == expected:
            passed += 1
        else:
            if failed > 0:
                print("==========")
            failed += 1
            command = case_input.get("tool_input", {}).get("command", "<no command>")
            print(f"FAIL: {command}")
            print(f"  expected: {expected}")
            print(f"  actual:   {actual}")
            if stdout.strip():
                print(f"  stdout:   {stdout.strip()}")
            if stderr.strip():
                print(f"  stderr:   {stderr.strip()}")

    total = passed + failed
    print(f"\n{passed} passed, {failed} failed, {total} total")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
