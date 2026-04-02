#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""
Redact PII and sensitive content from test-cases.json before it enters the repo.

Redaction targets:
- Home directory paths (replaced with /home/user)
- Path segments after the home dir (hashed to project-XXXXX / file-XXXXX.ext)
- Temporary file paths under /tmp/
- Git commit hashes (replaced with deterministic uppercase fakes)
- Commit messages and PR titles (quoted args to gaac/submit-pr)
- Timestamps (YYYYMMDD_HHMMSS patterns zeroed out)
- tool_input.description fields (removed entirely)
- Metadata fields: session_id, agent_id, transcript_path, etc.

Usage:
    ./redact.py           # Preview redacted output on stdout
    ./redact.py --apply   # Redact test-cases.json in place
    ./redact.py --audit   # Verify no suspicious content remains
"""

import hashlib
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

SAFE_PATH_SEGMENTS = {
    # Standard hidden dirs
    ".local", ".claude", ".venv", ".pants.d", ".config",
    # Python/standard
    "__pycache__", "bin", "lib", "site-packages",
    # Common project structure
    "tests", "scripts", "src", "tools",
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


def apply_recursive(obj, func):
    """Apply a string transformation function recursively to all string values."""
    if isinstance(obj, str):
        return func(obj)
    elif isinstance(obj, dict):
        return {k: apply_recursive(v, func) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [apply_recursive(item, func) for item in obj]
    return obj


def apply_replacements(obj, replacements: dict[str, str]):
    """Recursively apply string replacements, longest match first."""
    def replace_all(text):
        for old in sorted(replacements, key=len, reverse=True):
            text = text.replace(old, replacements[old])
        return text
    return apply_recursive(obj, replace_all)


def _hash_segment(segment: str) -> str:
    """Produce a deterministic 5-char uppercase hex hash of a path segment."""
    return hashlib.sha256(segment.encode()).hexdigest()[:5].upper()


def _generalize_home_path_tail(tail: str) -> str:
    """Generalize path segments after /home/user/.

    Segments in SAFE_PATH_SEGMENTS pass through unchanged.
    The last segment with a dot extension is treated as a filename.
    All other segments become project-XXXXX.
    """
    parts = tail.strip("/").split("/")
    result = []
    for i, part in enumerate(parts):
        if part in SAFE_PATH_SEGMENTS:
            result.append(part)
        elif i == len(parts) - 1 and "." in part:
            # Filename: hash basename, keep extension
            base, ext = part.rsplit(".", 1)
            result.append(f"file-{_hash_segment(base)}.{ext}")
        else:
            result.append(f"project-{_hash_segment(part)}")
    return "/".join(result)


def _generalize_tmp_path(filename: str) -> str:
    """Generalize a filename under /tmp/."""
    if "." in filename:
        base, ext = filename.rsplit(".", 1)
        return f"file-{_hash_segment(base)}.{ext}"
    return f"file-{_hash_segment(filename)}"


def generalize_path_segments(text: str) -> str:
    """Replace non-allowlisted path segments with deterministic hashed placeholders."""

    def replace_home_path(match: re.Match) -> str:
        tail = match.group(1)
        return "/home/user/" + _generalize_home_path_tail(tail)

    text = re.sub(
        r'/home/user/([^\s"\'|;&)>]+)',
        replace_home_path,
        text,
    )

    def replace_tmp_path(match: re.Match) -> str:
        filename = match.group(1)
        return "/tmp/" + _generalize_tmp_path(filename)

    text = re.sub(
        r'/tmp/([^\s"\'|;&)>]+)',
        replace_tmp_path,
        text,
    )

    return text


def redact_git_hashes(text: str) -> str:
    """Replace git commit hashes (7-40 lowercase hex chars) with deterministic fakes.

    Fake hashes use uppercase hex so they don't match the lowercase pattern on
    subsequent passes, making redaction idempotent.
    """

    def replace_hash(match: re.Match) -> str:
        original = match.group(0)
        return hashlib.sha256(original.encode()).hexdigest()[:len(original)].upper()

    return re.sub(r'\b[0-9a-f]{7,40}\b', replace_hash, text)


def redact_quoted_tool_args(text: str) -> str:
    """Redact quoted arguments after gaac and submit-pr commands."""
    # submit-pr: replace everything after the command with "<redacted>"
    text = re.sub(r'(submit-pr)\s+.+', r'\1 "<redacted>"', text, flags=re.DOTALL)
    # gaac: replace just the quoted argument
    text = re.sub(r'(gaac)\s+"[^"]*"', r'\1 "<redacted>"', text)
    return text


def redact_timestamps(text: str) -> str:
    """Replace timestamps like 20260312_113150 or 20260312-113150 with zeroes."""
    return re.sub(r'\d{8}[_-]\d{6}', '00000000_000000', text)


def redact_entry(entry: dict) -> dict:
    """Redact PII from a single test case entry."""
    # 1-2. Build and apply home-dir replacements
    replacements = build_replacements(entry)
    redacted = apply_replacements(entry, replacements)

    # 3. Remove top-level REMOVED_FIELDS
    for field in REMOVED_FIELDS:
        redacted.pop(field, None)

    # 4. Remove tool_input.description (Feature 1)
    tool_input = redacted.get("tool_input")
    if isinstance(tool_input, dict):
        tool_input.pop("description", None)

    # 5. Apply path segment generalization (Feature 3)
    redacted = apply_recursive(redacted, generalize_path_segments)

    # 6. Apply pattern-based redaction (Feature 2)
    redacted = apply_recursive(redacted, redact_timestamps)
    redacted = apply_recursive(redacted, redact_quoted_tool_args)
    redacted = apply_recursive(redacted, redact_git_hashes)

    return redacted


def redact_test_cases(test_cases: list) -> list:
    """Redact all test cases in the list."""
    return [[redact_entry(case), result] for case, result in test_cases]


def audit_entry(entry: dict, index: int) -> list[str]:
    """Scan a single entry for suspicious remaining content. Returns findings."""
    findings = []

    def collect_strings(obj, field_path=""):
        """Yield (field_path, string_value) for all strings in the structure."""
        if isinstance(obj, str):
            yield field_path, obj
        elif isinstance(obj, dict):
            for k, v in obj.items():
                yield from collect_strings(v, f"{field_path}.{k}" if field_path else k)
        elif isinstance(obj, list):
            for i, item in enumerate(obj):
                yield from collect_strings(item, f"{field_path}[{i}]")

    # Check 5: description field in tool_input
    tool_input = entry.get("tool_input")
    if isinstance(tool_input, dict) and "description" in tool_input:
        findings.append(
            f"[{index}] tool_input.description: still present"
        )

    for field_path, value in collect_strings(entry):
        # Check 1: Suspicious path segments below /home/user/
        for match in re.finditer(r'/home/user/([^\s"\'|;&)>]+)', value):
            tail = match.group(1)
            for segment in tail.split("/"):
                if not segment:
                    continue
                if segment in SAFE_PATH_SEGMENTS:
                    continue
                if re.fullmatch(r'project-[A-F0-9]{5}', segment):
                    continue
                if re.fullmatch(r'file-[A-F0-9]{5}\.\w+', segment):
                    continue
                findings.append(
                    f"[{index}] {field_path}: suspicious path segment '{segment}'"
                )

        # Check 2: Hex strings 7+ chars that could be git hashes
        for match in re.finditer(r'\b[0-9a-f]{7,40}\b', value):
            hex_str = match.group(0)
            # Skip if it's a fixed point of our hash redaction (already redacted)
            fake = hashlib.sha256(hex_str.encode()).hexdigest()[:len(hex_str)]
            if fake == hex_str:
                continue
            findings.append(
                f"[{index}] {field_path}: possible git hash '{hex_str}'"
            )

        # Check 3: Quoted strings after gaac/submit-pr that aren't redacted
        if field_path == "tool_input.command":
            for match in re.finditer(r'(?:gaac|submit-pr)\s+"([^"]*)"', value):
                quoted = match.group(1)
                if quoted == "<redacted>":
                    continue
                findings.append(
                    f"[{index}] {field_path}: long quoted string after tool command"
                )

        # Check 4: Timestamp patterns
        for match in re.finditer(r'\d{8}[_-]\d{6}', value):
            ts = match.group(0)
            if ts == "00000000_000000":
                continue
            findings.append(
                f"[{index}] {field_path}: timestamp '{ts}'"
            )

        # Check 6: Suspicious relative paths in commands
        if field_path == "tool_input.command":
            for match in re.finditer(r'(?<![/\w.\-])(\w[\w.-]*/[\w./-]+)', value):
                rel_path = match.group(1)
                # Skip URL-like strings (e.g., Go module paths) and git ranges
                if "://" in rel_path or "." in rel_path.split("/")[0] or ".." in rel_path:
                    continue
                segments = [s for s in rel_path.split("/") if s]
                if len(segments) < 2:
                    continue
                for segment in segments:
                    if segment.startswith("."):
                        continue
                    if segment in SAFE_PATH_SEGMENTS:
                        continue
                    if re.fullmatch(r'project-[A-F0-9]{5}', segment):
                        continue
                    if re.fullmatch(r'file-[A-F0-9]{5}\.\w+', segment):
                        continue
                    findings.append(
                        f"[{index}] {field_path}: suspicious relative path segment '{segment}'"
                    )

    return findings


def audit_test_cases(test_cases: list) -> list[str]:
    """Audit all test cases and return all findings."""
    all_findings = []
    for i, (case, _result) in enumerate(test_cases):
        all_findings.extend(audit_entry(case, index=i))
    return all_findings


def main():
    apply = "--apply" in sys.argv
    audit = "--audit" in sys.argv

    with open(TEST_CASES_FILE, "r") as f:
        test_cases = json.load(f)

    if audit:
        findings = audit_test_cases(test_cases)
        if findings:
            for finding in findings:
                print(finding)
            print(f"\n{len(findings)} issue(s) found.")
            sys.exit(1)
        else:
            print("Audit clean: no issues found.")
            sys.exit(0)

    redacted = redact_test_cases(test_cases)

    if apply:
        with open(TEST_CASES_FILE, "w") as f:
            json.dump(redacted, f, indent=4)
        print(f"Redacted {len(test_cases)} test cases in {TEST_CASES_FILE}")
    else:
        print(json.dumps(redacted, indent=4))


if __name__ == "__main__":
    main()
