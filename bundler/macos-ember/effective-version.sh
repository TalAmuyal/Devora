#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BASE_VERSION=$(cat "$REPO_ROOT/VERSION")
if git -C "$REPO_ROOT" describe --exact-match --tags --match 'v*' HEAD >/dev/null 2>&1; then
	EFFECTIVE_VERSION="$BASE_VERSION"
else
	LAST_TAG=$(git -C "$REPO_ROOT" describe --tags --match 'v*' --abbrev=0 2>/dev/null || echo "")
	if [ -n "$LAST_TAG" ]; then
		COMMIT_COUNT=$(git -C "$REPO_ROOT" rev-list --count "${LAST_TAG}..HEAD")
	else
		COMMIT_COUNT=$(git -C "$REPO_ROOT" rev-list --count HEAD)
	fi
	EFFECTIVE_VERSION="${BASE_VERSION}-dev.${COMMIT_COUNT}"
fi

printf "%s\n" "$EFFECTIVE_VERSION"
