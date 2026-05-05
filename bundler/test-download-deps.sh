#!/bin/bash

# Tests for extraction functions in download-deps.sh.
# Sources download-deps.sh and exercises extraction functions against
# locally-built fixtures — no network, no JSON parsing.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Stub variables that extract_tar_gz references when sourced.
TEMP_DIR=$(mktemp -d)
TARGET_DIR=$(mktemp -d)

cleanup() {
	rm -rf "$TEMP_DIR" "$TARGET_DIR"
}
trap cleanup EXIT

# shellcheck source=./download-deps.sh
source "$SCRIPT_DIR/download-deps.sh"

# shellcheck source=./test-helpers.sh
source "$SCRIPT_DIR/test-helpers.sh"

# ---- Test: extract a single file (existing behavior) ----
echo "test: extract_tar_gz with extract_type=file"
file_fixture_dir=$(mktemp -d)
echo "fake-binary-contents" > "$file_fixture_dir/myapp"
file_archive="$TEMP_DIR/file-fixture.tar.gz"
tar -czf "$file_archive" -C "$file_fixture_dir" myapp
rm -rf "$file_fixture_dir"

extract_tar_gz "myapp-test" "$file_archive" "myapp" "file"

assert_exists "extracted file is present" "$TARGET_DIR/myapp"
assert_eq "extracted file contents match" \
	"fake-binary-contents" \
	"$(cat "$TARGET_DIR/myapp")"
file_perms=$(stat -c '%a' "$TARGET_DIR/myapp" 2>/dev/null || stat -f '%Lp' "$TARGET_DIR/myapp")
assert_eq "extracted file is mode 755" "755" "$file_perms"

# ---- Test: extract a directory (new behavior) ----
echo "test: extract_tar_gz with extract_type=directory"
dir_fixture_root=$(mktemp -d)
mkdir -p "$dir_fixture_root/mydir/subdir"
echo "launcher-stub" > "$dir_fixture_root/mydir/launcher"
echo "nested-content" > "$dir_fixture_root/mydir/subdir/nested.txt"
dir_archive="$TEMP_DIR/dir-fixture.tar.gz"
tar -czf "$dir_archive" -C "$dir_fixture_root" mydir
rm -rf "$dir_fixture_root"

extract_tar_gz "mydir-test" "$dir_archive" "mydir" "directory"

assert_exists "extracted directory is present" "$TARGET_DIR/mydir"
assert_exists "top-level file inside directory" "$TARGET_DIR/mydir/launcher"
assert_exists "nested subdirectory" "$TARGET_DIR/mydir/subdir"
assert_exists "nested file" "$TARGET_DIR/mydir/subdir/nested.txt"
assert_eq "top-level file contents" \
	"launcher-stub" \
	"$(cat "$TARGET_DIR/mydir/launcher")"
assert_eq "nested file contents" \
	"nested-content" \
	"$(cat "$TARGET_DIR/mydir/subdir/nested.txt")"

# ---- Test: extract a raw binary (no archive wrapping) ----
echo "test: extract_raw"
raw_fixture_dir=$(mktemp -d)
echo "raw-binary-contents" > "$raw_fixture_dir/myraw"
raw_archive="$TEMP_DIR/raw-fixture"
cp "$raw_fixture_dir/myraw" "$raw_archive"
rm -rf "$raw_fixture_dir"

extract_raw "myraw-test" "$raw_archive" "myraw"

assert_exists "raw file is present" "$TARGET_DIR/myraw"
assert_eq "raw file contents match" \
	"raw-binary-contents" \
	"$(cat "$TARGET_DIR/myraw")"
raw_perms=$(stat -c '%a' "$TARGET_DIR/myraw" 2>/dev/null || stat -f '%Lp' "$TARGET_DIR/myraw")
assert_eq "raw file is mode 755" "755" "$raw_perms"

# ---- Test: extract a deeply-nested directory (depth 3+) ----
echo "test: extract_tar_gz with extract_type=directory at depth 3+"
deep_fixture_root=$(mktemp -d)
mkdir -p "$deep_fixture_root/outer/middle/target-dir"
echo "deep-content" > "$deep_fixture_root/outer/middle/target-dir/file.txt"
deep_archive="$TEMP_DIR/deep-fixture.tar.gz"
tar -czf "$deep_archive" -C "$deep_fixture_root" outer
rm -rf "$deep_fixture_root"

extract_tar_gz "deep-test" "$deep_archive" "target-dir" "directory"

assert_exists "deeply-nested directory is present" "$TARGET_DIR/target-dir"
assert_exists "file inside deeply-nested directory" "$TARGET_DIR/target-dir/file.txt"
assert_eq "deeply-nested file contents" \
	"deep-content" \
	"$(cat "$TARGET_DIR/target-dir/file.txt")"

# ---- Test: extraction functions do not write version markers ----
echo "test: extract_raw does not write a .dep-version marker"
assert_not_exists "no .dep-version after extract_raw" "$TARGET_DIR/myraw.dep-version"

echo "test: extract_tar_gz does not write a .dep-version marker"
assert_not_exists "no .dep-version after extract_tar_gz (file)" "$TARGET_DIR/myapp.dep-version"
assert_not_exists "no .dep-version after extract_tar_gz (directory)" "$TARGET_DIR/mydir.dep-version"

# ---- Test: check_version helper ----
echo "test: check_version returns 0 (skip) when target exists and marker matches"
cv_dir=$(mktemp -d)
echo "content" > "$cv_dir/artifact"
echo "1.2.3" > "$cv_dir/artifact.dep-version"
cv_rc=0
check_version "$cv_dir/artifact" "1.2.3" || cv_rc=$?
assert_eq "skip when version matches" "0" "$cv_rc"

echo "test: check_version returns 1 when target exists but marker is missing"
cv_dir2=$(mktemp -d)
echo "content" > "$cv_dir2/artifact"
cv_rc=0
check_version "$cv_dir2/artifact" "1.2.3" || cv_rc=$?
assert_eq "re-download when marker missing" "1" "$cv_rc"

echo "test: check_version returns 1 when target exists but marker has wrong version"
cv_dir3=$(mktemp -d)
echo "content" > "$cv_dir3/artifact"
echo "0.9.0" > "$cv_dir3/artifact.dep-version"
cv_rc=0
check_version "$cv_dir3/artifact" "1.2.3" || cv_rc=$?
assert_eq "re-download when version stale" "1" "$cv_rc"

echo "test: check_version returns 1 when target does not exist"
cv_dir4=$(mktemp -d)
cv_rc=0
check_version "$cv_dir4/nonexistent" "1.2.3" || cv_rc=$?
assert_eq "re-download when target missing" "1" "$cv_rc"

# ---- Summary ----
print_test_results
