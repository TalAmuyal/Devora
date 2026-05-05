#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""
Judge audit log statistics
===========================

Reads the JSONL audit log produced by Judge and prints
human-readable summary statistics to stdout.

Usage:
    ./audit-stats.py [--since 24h] [--decision allow] [--tool Bash] [--verbose]
"""
import argparse
import json
import pathlib
import re
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone


AUDIT_LOG_FILE = pathlib.Path.home() / ".claude" / "cc-judge-audit.jsonl"

DURATION_PATTERN = re.compile(r"^(\d+)([mhd])$")

DURATION_UNITS: dict[str, str] = {
    "m": "minutes",
    "h": "hours",
    "d": "days",
}


def parse_since(value: str) -> datetime:
    """Parse a --since value into a UTC datetime threshold.

    Accepts relative durations like '1h', '30m', '7d' or ISO dates like '2026-05-04'.
    """
    match = DURATION_PATTERN.match(value)
    if match:
        amount = int(match.group(1))
        unit = match.group(2)
        delta = timedelta(**{DURATION_UNITS[unit]: amount})
        return datetime.now(timezone.utc) - delta

    return datetime.fromisoformat(value).replace(tzinfo=timezone.utc)


def format_duration_label(value: str) -> str:
    """Turn a --since value into a human label like 'last 24h'."""
    match = DURATION_PATTERN.match(value)
    if match:
        return f"last {value}"
    return f"since {value}"


def load_entries(log_path: pathlib.Path) -> list[dict]:
    """Read every line of the JSONL file, skipping malformed lines."""
    entries: list[dict] = []
    with open(log_path) as f:
        for line_no, raw_line in enumerate(f, start=1):
            raw_line = raw_line.strip()
            if not raw_line:
                continue
            try:
                entries.append(json.loads(raw_line))
            except json.JSONDecodeError as exc:
                print(
                    f"WARNING: skipping malformed line {line_no}: {exc}",
                    file=sys.stderr,
                )
    return entries


def apply_filters(
    entries: list[dict],
    *,
    since: datetime | None,
    decision: str | None,
    tool: str | None,
) -> list[dict]:
    filtered: list[dict] = []
    for entry in entries:
        if since is not None:
            ts_raw = entry.get("ts")
            if ts_raw is None:
                continue
            ts = datetime.fromisoformat(ts_raw)
            if ts < since:
                continue
        if decision is not None and entry.get("decision") != decision:
            continue
        if tool is not None and entry.get("tool_name") != tool:
            continue
        filtered.append(entry)
    return filtered


def print_summary(entries: list[dict], header: str) -> None:
    print(header)
    print()

    decision_counts = Counter(entry.get("decision", "unknown") for entry in entries)
    reason_counts = Counter(entry.get("reason", "unknown") for entry in entries)
    session_counts = Counter(entry.get("session_id") or "(no session)" for entry in entries)
    total = len(entries)

    print("Decisions:")
    for name, count in decision_counts.most_common():
        pct = count / total * 100
        print(f"  {name:<8} {count:>4}  ({pct:4.1f}%)")
    print()

    print("Top reasons:")
    for name, count in reason_counts.most_common(10):
        print(f"  {name:<30} {count:>4}")
    print()

    print("Sessions:")
    for sid, count in session_counts.most_common():
        print(f"  {sid:<16} {count:>4} entries")
    print()

    # --- Average duration ---
    durations = [
        entry["duration_us"]
        for entry in entries
        if "duration_us" in entry
    ]
    if durations:
        avg_us = sum(durations) / len(durations)
        avg_ms = avg_us / 1000
        print(f"Avg duration: {avg_ms:.1f}ms")
    else:
        print("Avg duration: N/A (no duration data)")


def print_verbose(entries: list[dict]) -> None:
    for entry in entries:
        ts_raw = entry.get("ts", "?")
        # Trim sub-second precision for display
        try:
            ts_display = datetime.fromisoformat(ts_raw).strftime("%Y-%m-%dT%H:%M:%S")
        except (ValueError, TypeError):
            ts_display = ts_raw

        tool = entry.get("tool_name", "?")
        decision = entry.get("decision", "?")
        reason = entry.get("reason", "?")

        print("---")
        print(f"[{ts_display}] {tool} -> {decision} ({reason})")

        tool_input = entry.get("tool_input", {})
        if "command" in tool_input:
            print(f"  Command: {tool_input['command']}")

        trail = entry.get("trail")
        if trail:
            print("  Trail:")
            for breadcrumb in trail:
                print(f"    {breadcrumb}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Print summary statistics from the Judge audit log.",
    )
    parser.add_argument(
        "--since",
        metavar="DURATION_OR_DATE",
        help="Filter entries by time (e.g. 1h, 30m, 7d, 2026-05-04)",
    )
    parser.add_argument(
        "--decision",
        help="Filter by decision type (allow, deny, defer, error)",
    )
    parser.add_argument(
        "--tool",
        help="Filter by tool name (Bash, Read, WebFetch, ...)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print full trail for each matched entry after the summary",
    )
    parser.add_argument(
        "--log-file",
        type=pathlib.Path,
        help="Override the default log file path",
    )
    args = parser.parse_args()

    log_path: pathlib.Path = args.log_file if args.log_file is not None else AUDIT_LOG_FILE

    if not log_path.exists():
        print(f"Audit log not found: {log_path}", file=sys.stderr)
        sys.exit(1)

    entries = load_entries(log_path)

    since_dt: datetime | None = None
    if args.since is not None:
        since_dt = parse_since(args.since)

    filtered = apply_filters(
        entries,
        since=since_dt,
        decision=args.decision,
        tool=args.tool,
    )

    if not filtered:
        print("No matching entries found.")
        sys.exit(0)

    # Build header
    scope = "all time"
    if args.since is not None:
        scope = format_duration_label(args.since)
    header = f"Judge Audit Log ({scope}, {len(filtered)} entries)"

    print_summary(filtered, header)

    if args.verbose:
        print()
        print_verbose(filtered)


if __name__ == "__main__":
    main()
