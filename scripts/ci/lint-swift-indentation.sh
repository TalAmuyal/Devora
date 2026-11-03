#!/usr/bin/env bash
# Check handwritten Swift, including new files; ignored generated bindings/builds are excluded.
set -euo pipefail
cd "$(dirname "$0")/../.."
failed=0
while IFS= read -r -d '' file; do
	if grep -n '^ ' "$file"; then
		echo "ERROR: $file: use tabs for Swift indentation" >&2
		failed=1
	fi
done < <(git ls-files -z --cached --others --exclude-standard -- 'project-sapphire/**/*.swift' 'project-sapphire/*.swift')
exit "$failed"
