#!/usr/bin/env bash
# Prints one outcome per Rust test from `cargo test`'s output (libtest's `test <name> ... ok|FAILED|ignored` lines):
# {"suite", "name", "outcome", "duration"}, the rest from record.sh. Stable libtest reports no per-test time: duration is null.
# Identity (README.md): the suite is the test binary, by package: `<package>` for its unit tests, `<package>/<file>` for an
# integration test (cargo runs a package's targets together, its unit tests first), `<package> (doc)` for its doc-tests;
# the name is libtest's, a doc-test's without its `(line N)`, which moves when the file is edited.
# Lines may carry mise's `[task] ` prefix.
set -euo pipefail
sed -E 's/^\[[^]]*\] //; s/^test (.*) \.\.\. (ok|FAILED|ignored)(, .*)?$/TEST	\1	\2/' "$1" | awk -F '\t' '
	/^ *Running unittests / {
		binary = $0; sub(/\)$/, "", binary); sub(/.*\//, "", binary); sub(/-[0-9a-f]+$/, "", binary)
		package = binary; suite = package; next
	}
	/^ *Running / {
		file = $0; sub(/^ *Running /, "", file); sub(/ .*/, "", file); sub(/.*\//, "", file); sub(/\.rs$/, "", file)
		suite = package "/" file; next
	}
	/^ *Doc-tests / { doc = $0; sub(/^ *Doc-tests /, "", doc); suite = doc " (doc)"; next }
	$1 == "TEST" {
		name = $2
		sub(/ - should panic$/, "", name)
		sub(/ \(line [0-9]+\)$/, "", name)
		print suite "\t" name "\t" $3
	}
' | jq -c -R 'split("\t") | {suite: .[0], name: .[1], outcome: ({"ok": "passed", "FAILED": "failed"}[.[2]] // "skipped"), duration: null}'
