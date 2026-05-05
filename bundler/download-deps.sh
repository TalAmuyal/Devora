#!/bin/bash

set -euo pipefail

extract_tar_gz() {
	local name="$1"
	local archive="$2"
	local extract_path="$3"
	local extract_type="${4:-file}"

	local extract_dir="$TEMP_DIR/${name}-extract"
	mkdir -p "$extract_dir"

	echo "[$name] Extracting..."
	tar -xzf "$archive" -C "$extract_dir"

	local found
	case "$extract_type" in
		file)
			found=$(find "$extract_dir" -name "$extract_path" -type f | head -1)
			if [ -z "$found" ]; then
				echo "[$name] Error: could not find file '$extract_path' in archive"
				exit 1
			fi
			cp "$found" "$TARGET_DIR/$extract_path"
			chmod 755 "$TARGET_DIR/$extract_path"
			;;
		directory)
			found=$(find "$extract_dir" -name "$extract_path" -type d | head -1)
			if [ -z "$found" ]; then
				echo "[$name] Error: could not find directory '$extract_path' in archive"
				exit 1
			fi
			cp -R "$found" "$TARGET_DIR/$extract_path"
			;;
		*)
			echo "[$name] Error: unknown extract_type '$extract_type' (must be 'file' or 'directory')"
			exit 1
			;;
	esac
}

extract_raw() {
	local name="$1"
	local archive="$2"
	local extract_path="$3"

	echo "[$name] Copying..."
	cp "$archive" "$TARGET_DIR/$extract_path"
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

# Check whether a dependency is already present at the expected version.
# Returns 0 (skip download) when the target exists and its version marker matches.
# Returns 1 (re-download needed) otherwise.
check_version() {
	local target="$1"
	local version="$2"
	local version_file="${target}.dep-version"

	if [ -e "$target" ] && [ -f "$version_file" ] && [ "$(cat "$version_file")" = "$version" ]; then
		return 0
	fi
	return 1
}

main() {
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

	# Parse the JSON into a bash-friendly format using macOS system Python
	# Each line: name<TAB>version<TAB>download_url<TAB>sha256<TAB>archive_type<TAB>extract_path<TAB>extract_type
	local dep_lines
	dep_lines=$(/usr/bin/python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
for dep in data['dependencies']:
    print('\t'.join([
        dep['name'],
        dep['version'],
        dep['download_url'],
        dep['sha256'],
        dep['archive_type'],
        dep['extract_path'],
        dep.get('extract_type', 'file'),
    ]))
" "$DEPS_JSON")

	local name version download_url sha256 archive_type extract_path extract_type target local_archive
	while IFS=$'\t' read -r name version download_url sha256 archive_type extract_path extract_type; do
		target="$TARGET_DIR/$extract_path"

		if check_version "$target" "$version"; then
			echo "[$name] Already at version $version, skipping"
			continue
		fi
		if [ -e "$target" ]; then
			echo "[$name] Stale (expected $version), removing"
			rm -rf "$target"
			rm -f "${target}.dep-version"
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
				extract_tar_gz "$name" "$local_archive" "$extract_path" "$extract_type"
				;;
			dmg)
				extract_dmg "$name" "$local_archive" "$extract_path"
				;;
			raw)
				extract_raw "$name" "$local_archive" "$extract_path"
				;;
			*)
				echo "[$name] Error: unknown archive type '$archive_type'"
				exit 1
				;;
		esac

		echo "$version" > "${target}.dep-version"
		echo "[$name] Done"
	done <<< "$dep_lines"

	echo "All dependencies ready"
}

# Only run main() when this file is executed directly, not when sourced (e.g., from tests).
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
	main
fi
