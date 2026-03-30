#!/bin/bash

set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT="$SCRIPT_DIR"

VERSION_FILE="$REPO_ROOT/VERSION"
CHANGELOG_FILE="$REPO_ROOT/CHANGELOG.md"

# Read current version
if [ ! -f "$VERSION_FILE" ]; then
	echo "Error: VERSION file not found at $VERSION_FILE"
	exit 1
fi
CURRENT_VERSION=$(cat "$VERSION_FILE")

# Generate default release version based on today's date
TODAY=$(date +%Y-%m-%d)
if [ -n "$1" ]; then
	NEW_VERSION="$1"
else
	if echo "$CURRENT_VERSION" | grep -q "^${TODAY}\."; then
		CURRENT_PATCH=$(echo "$CURRENT_VERSION" | sed "s/^${TODAY}\\.//")
		NEW_PATCH=$((CURRENT_PATCH + 1))
		NEW_VERSION="${TODAY}.${NEW_PATCH}"
	else
		NEW_VERSION="${TODAY}.0"
	fi
fi

# Validate CHANGELOG.md exists
if [ ! -f "$CHANGELOG_FILE" ]; then
	echo "Error: CHANGELOG.md not found at $CHANGELOG_FILE"
	exit 1
fi

# Validate that the Unreleased section has at least one non-empty content line.
# Extract lines between "## Unreleased" and the next "##" heading (or EOF),
# excluding the heading lines themselves.
HAS_CONTENT=$(awk '
	/^## Unreleased$/ { in_unreleased = 1; next }
	in_unreleased && /^## / { exit }
	in_unreleased && /[^ \t]/ { print "true"; exit }
' "$CHANGELOG_FILE")

if [ "$HAS_CONTENT" != "true" ]; then
	echo "Error: The '## Unreleased' section in CHANGELOG.md has no content"
	exit 1
fi

# Transform CHANGELOG.md:
# Move content from under "## Unreleased" to under a new "## <version>" header.
# The "## Unreleased" line stays, followed by an empty section, then the new version header
# with the previous unreleased content.
awk -v version="$NEW_VERSION" '
	/^## Unreleased$/ {
		print $0
		print ""
		print "## " version
		in_unreleased = 1
		next
	}
	in_unreleased && /^## / {
		in_unreleased = 0
	}
	{ print }
' "$CHANGELOG_FILE" > "${CHANGELOG_FILE}.tmp"
mv "${CHANGELOG_FILE}.tmp" "$CHANGELOG_FILE"

# Write new version to VERSION file
echo -n "$NEW_VERSION" > "$VERSION_FILE"

echo "Release $NEW_VERSION prepared."
echo "Review the changes, then:"
echo "  git add VERSION CHANGELOG.md"
echo "  git commit -m 'Release $NEW_VERSION'"
echo "  git tag v$NEW_VERSION"
