#!/bin/bash

set -e

REPO_ROOT=$(cd "$(dirname "$0")" && pwd)

VERSION_FILE="$REPO_ROOT/VERSION"
CHANGELOG_FILE="$REPO_ROOT/CHANGELOG.md"

cd "$REPO_ROOT"

die() {
	echo "Error: $1" >&2
	exit 1
}

usage() {
	cat <<EOF
Usage: $(basename "$0") [OPTIONS] [VERSION]

Cut a new release of Devora.

Arguments:
  VERSION          Explicit version number (optional, auto-computed as YYYY-MM-DD.N if omitted)

Options:
  --tag-only       Tag-only recovery mode: read version from VERSION file and tag the merge commit
  --no-ai          Skip the Claude Code editorial cleanup step
  -h, --help       Show this help message

Examples:
  $(basename "$0")                  # Auto-compute version and cut release
  $(basename "$0") 2026-04-10.2    # Cut release with explicit version
  $(basename "$0") --no-ai         # Cut release without AI editorial cleanup
  $(basename "$0") --tag-only      # Tag the merge commit for the version in VERSION file
EOF
	exit 0
}

# ── Argument parsing ──────────────────────────────────────────────

TAG_ONLY=false
NO_AI=false
EXPLICIT_VERSION=""

while [ $# -gt 0 ]; do
	case "$1" in
		-h|--help)
			usage
			;;
		--tag-only)
			TAG_ONLY=true
			shift
			;;
		--no-ai)
			NO_AI=true
			shift
			;;
		-*)
			die "Unknown option: $1 (see --help)"
			;;
		*)
			[ -z "$EXPLICIT_VERSION" ] || die "Unexpected argument: $1 (version already set to '$EXPLICIT_VERSION')"
			EXPLICIT_VERSION="$1"
			shift
			;;
	esac
done

# ── Tag-only mode ─────────────────────────────────────────────────

if [ "$TAG_ONLY" = "true" ]; then
	[ -z "$EXPLICIT_VERSION" ] || die "--tag-only reads version from VERSION file and does not accept a version argument"

	[ -f "$VERSION_FILE" ] || die "VERSION file not found at $VERSION_FILE"
	VERSION=$(cat "$VERSION_FILE")
	[ -n "$VERSION" ] || die "VERSION file is empty"

	# Fetch first so tag check sees remote state
	git fetch origin

	# Check tag doesn't already exist
	git tag -l "v${VERSION}" | grep -q . && die "Tag v${VERSION} already exists"
	MERGE_SHA=$(git log --grep="^Release ${VERSION}" --format="%H" origin/master | head -1)
	[ -n "$MERGE_SHA" ] || die "Could not find merge commit for 'Release ${VERSION}' on master. Make sure the PR was merged."

	# Create and push tag
	git tag "v${VERSION}" "$MERGE_SHA"
	git push origin "v${VERSION}"

	echo "Tag v${VERSION} pushed at ${MERGE_SHA}."
	echo "The CD pipeline will build and publish the release."
	exit 0
fi

# ── Pre-flight checks ────────────────────────────────────────────

# Working tree is clean
git diff --quiet && git diff --cached --quiet || die "Working tree is dirty. Commit or stash changes first."

# Up to date with origin/master
git fetch origin
LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse origin/master)
[ "$LOCAL" = "$REMOTE" ] || die "Local is not up to date with origin/master."

# gh CLI is available and authenticated
command -v gh >/dev/null 2>&1 || die "gh CLI is required but not installed"
gh auth status >/dev/null 2>&1 || die "gh CLI is not authenticated. Run 'gh auth login'"

# ── Prepare ───────────────────────────────────────────────────────

[ -f "$VERSION_FILE" ] || die "VERSION file not found at $VERSION_FILE"
CURRENT_VERSION=$(cat "$VERSION_FILE")

# Generate default release version based on today's date
TODAY=$(date +%Y-%m-%d)
if [ -n "$EXPLICIT_VERSION" ]; then
	NEW_VERSION="$EXPLICIT_VERSION"
else
	if echo "$CURRENT_VERSION" | grep -q "^${TODAY}\."; then
		CURRENT_PATCH=$(echo "$CURRENT_VERSION" | sed "s/^${TODAY}\\.//")
		NEW_PATCH=$((CURRENT_PATCH + 1))
		NEW_VERSION="${TODAY}.${NEW_PATCH}"
	else
		NEW_VERSION="${TODAY}.0"
	fi
fi

# Check that tag doesn't already exist
git tag -l "v${NEW_VERSION}" | grep -q . && die "Tag v${NEW_VERSION} already exists"

# Validate CHANGELOG.md exists
[ -f "$CHANGELOG_FILE" ] || die "CHANGELOG.md not found at $CHANGELOG_FILE"

# Validate that the Unreleased section has at least one non-empty content line.
# Extract lines between "## Unreleased" and the next "##" heading (or EOF),
# excluding the heading lines themselves.
HAS_CONTENT=$(awk '
	/^## Unreleased$/ { in_unreleased = 1; next }
	in_unreleased && /^## / { exit }
	in_unreleased && /[^ \t]/ { print "true"; exit }
' "$CHANGELOG_FILE")

if [ "$HAS_CONTENT" != "true" ]; then
	die "The '## Unreleased' section in CHANGELOG.md has no content"
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

# ── AI editorial cleanup ──────────────────────────────────────────

if [ "$NO_AI" != "true" ]; then
	echo "Running editorial cleanup on changelog..."
	"$REPO_ROOT/ccc.sh" \
		-p \
		--permission-mode acceptEdits \
		"We are cutting release ${NEW_VERSION} now, use the rules for cutting a release (@docs/changlog-edits.md) to update the \`## ${NEW_VERSION}\` section of ./CHANGELOG.md"
fi

# ── Pause for review ─────────────────────────────────────────────

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Version: $NEW_VERSION"
echo "  CHANGELOG.md and VERSION have been updated."
echo ""
echo "  Please review the changes now (e.g., git diff)."
echo "  Press ENTER to continue to PR creation."
echo "  Press Ctrl-C to abort (revert with: git checkout -- VERSION CHANGELOG.md)"
echo "════════════════════════════════════════════════════════════"
read -r

# ── Ship ──────────────────────────────────────────────────────────

# Create branch
BRANCH="release/${NEW_VERSION}"
git checkout -b "$BRANCH"

# Stage and commit
git add VERSION CHANGELOG.md
git commit -m "Release ${NEW_VERSION}"

# Push
git push -u origin "$BRANCH"

# Extract release notes (content between ## <version> and the next ## header)
RELEASE_NOTES=$(awk -v ver="## ${NEW_VERSION}" \
	'$0 == ver { found=1; next } /^## / { found=0 } found { print }' \
	CHANGELOG.md)

# Create PR
PR_URL=$(gh pr create \
	--base master \
	--title "Release ${NEW_VERSION}" \
	--body "$(cat <<EOF
## Release ${NEW_VERSION}

${RELEASE_NOTES}
EOF
)")

# Return to master
git checkout origin/master

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  PR created: ${PR_URL}"
echo ""
echo "  Next steps:"
echo "  1. Review and approve the PR"
echo "  2. Squash-merge the PR"
echo "  3. The auto-tag workflow will handle tagging and release"
echo "════════════════════════════════════════════════════════════"
