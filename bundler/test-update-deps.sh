#!/bin/bash

# Tests for pure functions in update-deps.sh.
# Sources update-deps.sh and exercises utility functions against
# locally-built fixtures — no network, no JSON parsing.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Stub variables that the sourced script might reference.
TEMP_DIR=$(mktemp -d)
TARGET_DIR=$(mktemp -d)

cleanup() {
	rm -rf "$TEMP_DIR" "$TARGET_DIR"
}
trap cleanup EXIT

# shellcheck source=./update-deps.sh
source "$SCRIPT_DIR/update-deps.sh"

# shellcheck source=./test-helpers.sh
source "$SCRIPT_DIR/test-helpers.sh"

# ---- Test: extract_repo_slug ----
echo "test: extract_repo_slug"

assert_eq "standard github url" \
	"kovidgoyal/kitty" \
	"$(extract_repo_slug "https://github.com/kovidgoyal/kitty")"

assert_eq "url with trailing slash" \
	"owner/repo" \
	"$(extract_repo_slug "https://github.com/owner/repo/")"

# ---- Test: derive_version_from_tag ----
echo "test: derive_version_from_tag"

assert_eq "tag with v prefix" \
	"0.46.2" \
	"$(derive_version_from_tag "v0.46.2")"

assert_eq "tag without v prefix" \
	"0.10.12" \
	"$(derive_version_from_tag "0.10.12")"

assert_eq "v prefix edge case" \
	"1.0.0" \
	"$(derive_version_from_tag "v1.0.0")"

# ---- Test: compute_new_url ----
echo "test: compute_new_url"

assert_eq "version in filename (glow pattern)" \
	"https://github.com/charmbracelet/glow/releases/download/v2.2.0/glow_2.2.0_Darwin_arm64.tar.gz" \
	"$(compute_new_url \
		"https://github.com/charmbracelet/glow/releases/download/v2.1.1/glow_2.1.1_Darwin_arm64.tar.gz" \
		"v2.1.1" "v2.2.0" \
		"2.1.1" "2.2.0")"

assert_eq "no version in filename (uv pattern)" \
	"https://github.com/astral-sh/uv/releases/download/0.11.0/uv-aarch64-apple-darwin.tar.gz" \
	"$(compute_new_url \
		"https://github.com/astral-sh/uv/releases/download/0.10.12/uv-aarch64-apple-darwin.tar.gz" \
		"0.10.12" "0.11.0" \
		"0.10.12" "0.11.0")"

assert_eq "source archive (crit-cc-plugin pattern)" \
	"https://github.com/tomasz-tomczyk/crit/archive/refs/tags/v0.11.0.tar.gz" \
	"$(compute_new_url \
		"https://github.com/tomasz-tomczyk/crit/archive/refs/tags/v0.10.4.tar.gz" \
		"v0.10.4" "v0.11.0" \
		"0.10.4" "0.11.0")"

assert_eq "patch bump with prefix overlap" \
	"https://example.com/releases/download/v2.0.10/app-2.0.10.tar.gz" \
	"$(compute_new_url \
		"https://example.com/releases/download/v2.0.9/app-2.0.9.tar.gz" \
		"v2.0.9" "v2.0.10" \
		"2.0.9" "2.0.10")"

# ---- Test: apply_json_updates ----
echo "test: apply_json_updates"

fixture_json="$TEMP_DIR/test-deps.json"
changes_tsv="$TEMP_DIR/test-changes.tsv"

cat > "$fixture_json" <<'FIXTURE'
{
  "dependencies": [
    {
      "name": "foo",
      "version": "1.0.0",
      "tag": "v1.0.0",
      "download_url": "https://example.com/foo-1.0.0.tar.gz",
      "sha256": "aaa",
      "extract_path": "foo"
    },
    {
      "name": "bar",
      "version": "2.0.0",
      "tag": "v2.0.0",
      "download_url": "https://example.com/bar-2.0.0.tar.gz",
      "sha256": "bbb",
      "commit": "abc123",
      "source_tree_url": "https://github.com/x/bar/tree/abc123",
      "extract_path": "bar"
    }
  ]
}
FIXTURE

# Changes TSV: update foo only (12 fields)
printf 'foo\t2.0.0\tv2.0.0\thttps://example.com/foo-2.0.0.tar.gz\tccc\t\t\t1.0.0\t\t\tfoo\thttps://github.com/x/foo\n' > "$changes_tsv"

apply_json_updates "$fixture_json" "$changes_tsv"

# Verify with python3
result=$(/usr/bin/python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
deps = {d['name']: d for d in data['dependencies']}
foo = deps['foo']
bar = deps['bar']
print(foo['version'])
print(foo['tag'])
print(foo['sha256'])
print(bar['version'])
print(bar['sha256'])
" "$fixture_json")

foo_version=$(echo "$result" | sed -n '1p')
foo_tag=$(echo "$result" | sed -n '2p')
foo_sha256=$(echo "$result" | sed -n '3p')
bar_version=$(echo "$result" | sed -n '4p')
bar_sha256=$(echo "$result" | sed -n '5p')

assert_eq "foo version updated" "2.0.0" "$foo_version"
assert_eq "foo tag updated" "v2.0.0" "$foo_tag"
assert_eq "foo sha256 updated" "ccc" "$foo_sha256"
assert_eq "bar version unchanged" "2.0.0" "$bar_version"
assert_eq "bar sha256 unchanged" "bbb" "$bar_sha256"

# Verify JSON is valid
/usr/bin/python3 -c "import json, sys; json.load(open(sys.argv[1]))" "$fixture_json"
assert_eq "output JSON is valid" "0" "$?"

# ---- Test: apply_json_updates with commit fields ----
echo "test: apply_json_updates with commit fields"

fixture_json_commit="$TEMP_DIR/test-deps-commit.json"
changes_tsv_commit="$TEMP_DIR/test-changes-commit.tsv"

cat > "$fixture_json_commit" <<'FIXTURE'
{
  "dependencies": [
    {
      "name": "myapp",
      "version": "1.0.0",
      "tag": "v1.0.0",
      "download_url": "https://example.com/myapp-1.0.0.tar.gz",
      "sha256": "old_sha",
      "commit": "aaa111bbb",
      "source_tree_url": "https://github.com/x/myapp/tree/aaa111bbb",
      "extract_path": "myapp"
    }
  ]
}
FIXTURE

# Changes TSV with commit and source_tree_url (12 fields)
printf 'myapp\t2.0.0\tv2.0.0\thttps://example.com/myapp-2.0.0.tar.gz\tnew_sha\tccc333ddd\thttps://github.com/x/myapp/tree/ccc333ddd\t1.0.0\taaa111bbb\thttps://github.com/x/myapp/tree/aaa111bbb\tmyapp\thttps://github.com/x/myapp\n' > "$changes_tsv_commit"

apply_json_updates "$fixture_json_commit" "$changes_tsv_commit"

result=$(/usr/bin/python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
dep = data['dependencies'][0]
print(dep['version'])
print(dep['commit'])
print(dep['source_tree_url'])
" "$fixture_json_commit")

commit_version=$(echo "$result" | sed -n '1p')
commit_value=$(echo "$result" | sed -n '2p')
source_tree_url=$(echo "$result" | sed -n '3p')

assert_eq "version updated with commit" "2.0.0" "$commit_version"
assert_eq "commit updated" "ccc333ddd" "$commit_value"
assert_eq "source_tree_url updated" "https://github.com/x/myapp/tree/ccc333ddd" "$source_tree_url"

# ---- Test: apply_licenses_updates ----
echo "test: apply_licenses_updates"

fixture_licenses="$TEMP_DIR/test-licenses.md"
changes_tsv_lic="$TEMP_DIR/test-changes-lic.tsv"

cat > "$fixture_licenses" <<'FIXTURE'
## Bundled Binaries

### foo v1.0.0

- **License:** MIT
- **Source:** https://github.com/x/foo

### bar v2.0.0

- **License:** MIT
- **Source:** https://github.com/x/bar/tree/abc123def
FIXTURE

# Changes TSV: update foo to v3.0.0, bar to v3.0.0 with commit update
# Fields: name, new_version, new_tag, new_url, new_sha256, new_commit, new_source_tree_url,
#         old_version, old_commit, old_source_tree_url, extract_path, source_repo
printf 'foo\t3.0.0\tv3.0.0\thttps://example.com/foo-3.0.0.tar.gz\tddd\t\t\t1.0.0\t\t\tfoo\thttps://github.com/x/foo\n' > "$changes_tsv_lic"
printf 'bar\t3.0.0\tv3.0.0\thttps://example.com/bar-3.0.0.tar.gz\teee\txyz789abc\t\t2.0.0\tabc123def\t\tbar\thttps://github.com/x/bar\n' >> "$changes_tsv_lic"

apply_licenses_updates "$fixture_licenses" "$changes_tsv_lic"

licenses_content=$(cat "$fixture_licenses")

# Check foo heading updated
if echo "$licenses_content" | grep -qF "### foo v3.0.0"; then
	PASS=$((PASS + 1))
	echo "  ok: foo heading updated to v3.0.0"
else
	FAIL=$((FAIL + 1))
	echo "  FAIL: foo heading updated to v3.0.0"
	echo "    content: $licenses_content"
fi

# Check bar heading updated
if echo "$licenses_content" | grep -qF "### bar v3.0.0"; then
	PASS=$((PASS + 1))
	echo "  ok: bar heading updated to v3.0.0"
else
	FAIL=$((FAIL + 1))
	echo "  FAIL: bar heading updated to v3.0.0"
	echo "    content: $licenses_content"
fi

# Check bar source URL has new abbreviated commit
if echo "$licenses_content" | grep -qF "https://github.com/x/bar/tree/xyz789abc"; then
	PASS=$((PASS + 1))
	echo "  ok: bar source URL updated with new commit"
else
	FAIL=$((FAIL + 1))
	echo "  FAIL: bar source URL updated with new commit"
	echo "    content: $licenses_content"
fi

# Check old foo heading is gone
if echo "$licenses_content" | grep -qF "### foo v1.0.0"; then
	FAIL=$((FAIL + 1))
	echo "  FAIL: old foo heading should be gone"
else
	PASS=$((PASS + 1))
	echo "  ok: old foo heading is gone"
fi

# ---- Summary ----
print_test_results
