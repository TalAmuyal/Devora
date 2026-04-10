#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEPS_JSON="$SCRIPT_DIR/3rd-party-deps.json"
TARGET_DIR="$SCRIPT_DIR/macos/3rd-party-apps"

mkdir -p "$TARGET_DIR"

TEMP_DIR=$(mktemp -d)
DMG_MOUNT_POINT=""

cleanup() {
	if [ -n "$DMG_MOUNT_POINT" ] && [ -d "$DMG_MOUNT_POINT" ]; then
		hdiutil detach "$DMG_MOUNT_POINT" -quiet 2>/dev/null || true
	fi
	rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

extract_tar_gz() {
	local name="$1"
	local archive="$2"
	local extract_path="$3"

	local extract_dir="$TEMP_DIR/${name}-extract"
	mkdir -p "$extract_dir"

	echo "[$name] Extracting..."
	tar -xzf "$archive" -C "$extract_dir"

	local found
	found=$(find "$extract_dir" -name "$extract_path" -type f | head -1)
	if [ -z "$found" ]; then
		echo "[$name] Error: could not find '$extract_path' in archive"
		exit 1
	fi

	cp "$found" "$TARGET_DIR/$extract_path"
	chmod 755 "$TARGET_DIR/$extract_path"
}

extract_dmg() {
	local name="$1"
	local archive="$2"
	local extract_path="$3"

	DMG_MOUNT_POINT="$TEMP_DIR/${name}-dmg-mount"
	mkdir -p "$DMG_MOUNT_POINT"

	echo "[$name] Mounting DMG..."
	hdiutil attach "$archive" -nobrowse -noverify -noautoopen -mountpoint "$DMG_MOUNT_POINT"

	local src="$DMG_MOUNT_POINT/$extract_path"
	if [ ! -e "$src" ]; then
		echo "[$name] Error: could not find '$extract_path' in DMG"
		exit 1
	fi

	echo "[$name] Copying $extract_path..."
	rm -rf "$TARGET_DIR/$extract_path"
	cp -R "$src" "$TARGET_DIR/$extract_path"

	echo "[$name] Stripping quarantine attribute..."
	xattr -rd com.apple.quarantine "$TARGET_DIR/$extract_path" 2>/dev/null || true

	echo "[$name] Detaching DMG..."
	hdiutil detach "$DMG_MOUNT_POINT" -quiet
	DMG_MOUNT_POINT=""
}

verify_checksum() {
	local name="$1"
	local file="$2"
	local expected="$3"

	echo "[$name] Verifying checksum..."
	local actual
	actual=$(shasum -a 256 "$file" | awk '{print $1}')
	if [ "$actual" != "$expected" ]; then
		echo "[$name] Error: checksum mismatch"
		echo "[$name]   Expected: $expected"
		echo "[$name]   Actual:   $actual"
		exit 1
	fi
}

# Parse the JSON into a bash-friendly format using macOS system Python
# Each line: name<TAB>download_url<TAB>sha256<TAB>archive_type<TAB>extract_path
dep_lines=$(/usr/bin/python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
for dep in data['dependencies']:
    print('\t'.join([dep['name'], dep['download_url'], dep['sha256'], dep['archive_type'], dep['extract_path']]))
" "$DEPS_JSON")

while IFS=$'\t' read -r name download_url sha256 archive_type extract_path; do
	target="$TARGET_DIR/$extract_path"

	# Check if already present
	# Note: the sha256 in the JSON is for the archive, not the extracted file,
	# so we can only do an existence check here.
	if [ -e "$target" ]; then
		echo "[$name] Already exists, skipping"
		continue
	fi

	# Download
	local_archive="$TEMP_DIR/${name}-archive"
	echo "[$name] Downloading..."
	curl -fSL -o "$local_archive" "$download_url"

	# Verify checksum of downloaded archive
	verify_checksum "$name" "$local_archive" "$sha256"

	# Extract
	case "$archive_type" in
		tar.gz)
			extract_tar_gz "$name" "$local_archive" "$extract_path"
			;;
		dmg)
			extract_dmg "$name" "$local_archive" "$extract_path"
			;;
		*)
			echo "[$name] Error: unknown archive type '$archive_type'"
			exit 1
			;;
	esac

	echo "[$name] Done"
done <<< "$dep_lines"

echo "All dependencies ready"
