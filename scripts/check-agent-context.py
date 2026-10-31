#!/usr/bin/env python3
"""Require a Codex instruction file beside every tracked Claude context file."""

import re
import subprocess
from pathlib import Path


def main() -> int:
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
    contexts = [Path(name) for name in tracked if name and Path(name).name == "CLAUDE.md"]
    pointer = re.compile(r"\[[^\]\n]+\]\((?:\./)?CLAUDE\.md\)")
    errors = []
    for context in contexts:
        agent = context.with_name("AGENTS.md")
        if not (root / agent).is_file():
            errors.append(f"{context}: missing adjacent {agent}")
        elif not pointer.search((root / agent).read_text(encoding="utf-8")):
            errors.append(f"{agent}: must contain a relative Markdown link to CLAUDE.md")
    for error in errors:
        print(error)
    if errors:
        return 1
    print(f"Verified AGENTS.md pointers for {len(contexts)} CLAUDE.md files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
