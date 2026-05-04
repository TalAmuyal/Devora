#!/bin/bash

set -euo pipefail

extract_repo_slug() {
	local url="$1"
	url="${url%/}"
	local repo
	repo="${url##*/}"
	local owner_part="${url%/*}"
	local owner="${owner_part##*/}"
	echo "${owner}/${repo}"
}

derive_version_from_tag() {
	local tag="$1"
	echo "${tag#v}"
}

# Replace tag before version: the version is a substring of the tag (e.g. "0.10.4" in "v0.10.4"),
# so replacing version first would corrupt the tag inside the URL.
compute_new_url() {
	local old_url="$1"
	local old_tag="$2"
	local new_tag="$3"
	local old_version="$4"
	local new_version="$5"

	local result="${old_url//${old_tag}/${new_tag}}"
	result="${result//${old_version}/${new_version}}"
	echo "$result"
}

# Each line in the changes file has 12 tab-separated fields:
#   name, new_version, new_tag, new_url, new_sha256, new_commit, new_source_tree_url,
#   old_version, old_commit, old_source_tree_url, extract_path, source_repo
apply_json_updates() {
	local deps_path="$1"
	local changes_file="$2"
	/usr/bin/python3 -c "
import json, os, sys, tempfile

changes = []
with open(sys.argv[1]) as f:
    for line in f:
        fields = line.rstrip('\n').split('\t')
        changes.append({
            'name': fields[0],
            'new_version': fields[1],
            'new_tag': fields[2],
            'new_url': fields[3],
            'new_sha256': fields[4],
            'new_commit': fields[5],
            'new_source_tree_url': fields[6],
        })

deps_path = sys.argv[2]
with open(deps_path) as f:
    data = json.load(f)

changes_by_name = {c['name']: c for c in changes}
for dep in data['dependencies']:
    change = changes_by_name.get(dep['name'])
    if change is None:
        continue
    dep['version'] = change['new_version']
    dep['tag'] = change['new_tag']
    dep['download_url'] = change['new_url']
    dep['sha256'] = change['new_sha256']
    if change['new_commit'] and 'commit' in dep:
        dep['commit'] = change['new_commit']
    if change['new_source_tree_url'] and 'source_tree_url' in dep:
        dep['source_tree_url'] = change['new_source_tree_url']

fd, tmp = tempfile.mkstemp(dir=os.path.dirname(deps_path))
with os.fdopen(fd, 'w') as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write('\n')
os.replace(tmp, deps_path)
" "$changes_file" "$deps_path"
}

apply_licenses_updates() {
	local licenses_path="$1"
	local changes_file="$2"
	/usr/bin/python3 -c "
import sys

changes = []
with open(sys.argv[1]) as f:
    for line in f:
        fields = line.rstrip('\n').split('\t')
        changes.append({
            'name': fields[0],
            'new_version': fields[1],
            'old_version': fields[7],
            'old_commit': fields[8],
            'new_commit': fields[5],
            'old_source_tree_url': fields[9],
            'new_source_tree_url': fields[6],
            'source_repo': fields[11],
        })

licenses_path = sys.argv[2]
with open(licenses_path) as f:
    content = f.read()

for change in changes:
    name = change['name']
    # Skip crit-cc-plugin (shares crit's entry)
    if name == 'crit-cc-plugin':
        continue

    old_heading = '### ' + name + ' v' + change['old_version']
    new_heading = '### ' + name + ' v' + change['new_version']
    content = content.replace(old_heading, new_heading)

    if change['new_source_tree_url'] and change['old_source_tree_url']:
        content = content.replace(change['old_source_tree_url'], change['new_source_tree_url'])
    elif change['new_commit'] and change['old_commit']:
        old_source_line = change['source_repo'] + '/tree/' + change['old_commit'][:9]
        new_source_line = change['source_repo'] + '/tree/' + change['new_commit'][:9]
        content = content.replace(old_source_line, new_source_line)

with open(licenses_path, 'w') as f:
    f.write(content)
" "$changes_file" "$licenses_path"
}

main() {
	SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
	DEPS_JSON="$SCRIPT_DIR/3rd-party-deps.json"
	TARGET_DIR="$SCRIPT_DIR/macos/3rd-party-apps"
	LICENSES_FILE="$SCRIPT_DIR/../THIRD_PARTY_LICENSES.md"

	if ! command -v gh &>/dev/null; then
		echo "Error: gh CLI is required but not found. Install it from https://cli.github.com/"
		exit 1
	fi

	if ! gh auth status &>/dev/null; then
		echo "Error: gh CLI is not authenticated. Run 'gh auth login' first."
		exit 1
	fi

	TEMP_DIR=$(mktemp -d)
	cleanup() {
		rm -rf "$TEMP_DIR"
	}
	trap cleanup EXIT

	local dep_lines
	dep_lines=$(/usr/bin/python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
for dep in data['dependencies']:
    print('\t'.join([
        dep['name'],
        dep['version'],
        dep['tag'],
        dep['source_repo'],
        dep['download_url'],
        dep['sha256'],
        dep['extract_path'],
        dep.get('commit', ''),
        dep.get('source_tree_url', ''),
    ]))
" "$DEPS_JSON")

	# bash 3.2 lacks associative arrays
	local REPO_TAGS_FILE="$TEMP_DIR/repo-tags.tsv"
	: > "$REPO_TAGS_FILE"
	local seen_repos=""

	while IFS=$'\t' read -r name _ tag source_repo _ _ _ _ _; do
		local slug
		slug=$(extract_repo_slug "$source_repo")
		if ! echo "$seen_repos" | grep -qxF "$slug"; then
			seen_repos="${seen_repos}${slug}"$'\n'
			local latest_tag
			latest_tag=$(gh api "repos/${slug}/releases/latest" --jq '.tag_name')
			printf '%s\t%s\n' "$slug" "$latest_tag" >> "$REPO_TAGS_FILE"
		fi
	done <<< "$dep_lines"

	local CHANGES_FILE="$TEMP_DIR/changes.tsv"
	: > "$CHANGES_FILE"

	local skipped_names=()
	local skipped_tags=()
	local download_failed=false

	while IFS=$'\t' read -r name old_version old_tag source_repo old_url old_sha256 extract_path old_commit old_source_tree_url; do
		local slug
		slug=$(extract_repo_slug "$source_repo")
		local new_tag
		new_tag=$(grep "^${slug}	" "$REPO_TAGS_FILE" | cut -f2)

		if [ "$old_tag" = "$new_tag" ]; then
			echo "[$name] Already up to date ($old_tag)"
			skipped_names+=("$name")
			skipped_tags+=("$old_tag")
			continue
		fi

		local new_version
		new_version=$(derive_version_from_tag "$new_tag")
		local new_url
		new_url=$(compute_new_url "$old_url" "$old_tag" "$new_tag" "$old_version" "$new_version")

		echo "[$name] Checking $old_tag -> $new_tag"

		local local_archive="$TEMP_DIR/${name}-archive"
		echo "[$name] Downloading from $new_url..."
		if ! curl -fSL -o "$local_archive" "$new_url"; then
			echo "[$name] Error: download failed"
			download_failed=true
			continue
		fi

		local new_sha256
		new_sha256=$(shasum -a 256 "$local_archive" | awk '{print $1}')
		echo "[$name] SHA256: $new_sha256"

		local new_commit=""
		local new_source_tree_url=""
		if [ -n "$old_commit" ]; then
			echo "[$name] Resolving commit for tag $new_tag..."
			new_commit=$(gh api "repos/${slug}/commits/${new_tag}" --jq '.sha')

			if [ -n "$old_source_tree_url" ]; then
				new_source_tree_url="${source_repo}/tree/${new_commit}"
			fi
		fi

		printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
			"$name" "$new_version" "$new_tag" "$new_url" "$new_sha256" \
			"$new_commit" "$new_source_tree_url" \
			"$old_version" "$old_commit" "$old_source_tree_url" \
			"$extract_path" "$source_repo" \
			>> "$CHANGES_FILE"
	done <<< "$dep_lines"

	if [ "$download_failed" = true ]; then
		echo ""
		echo "Error: one or more downloads failed. Aborting without applying any changes."
		exit 1
	fi

	if [ -s "$CHANGES_FILE" ]; then
		echo ""
		echo "Applying updates..."

		apply_json_updates "$DEPS_JSON" "$CHANGES_FILE"

		while IFS=$'\t' read -r name _ _ _ _ _ _ _ _ _ extract_path _; do
			local old_artifact="$TARGET_DIR/$extract_path"
			if [ -e "$old_artifact" ]; then
				echo "[$name] Removing old artifact: $extract_path"
				rm -rf "$old_artifact"
			fi
		done < "$CHANGES_FILE"

		if [ -f "$LICENSES_FILE" ]; then
			apply_licenses_updates "$LICENSES_FILE" "$CHANGES_FILE"
		fi

		echo ""
		echo "Running download-deps.sh to fetch updated dependencies..."
		bash "$SCRIPT_DIR/download-deps.sh"
	fi

	echo ""
	echo "=== Update Summary ==="
	if [ -s "$CHANGES_FILE" ]; then
		while IFS=$'\t' read -r name new_version _ _ _ _ _ old_version _ _ _ _; do
			echo "  Updated: $name ($old_version -> $new_version)"
		done < "$CHANGES_FILE"
	fi
	for i in "${!skipped_names[@]}"; do
		echo "  Up to date: ${skipped_names[$i]} (${skipped_tags[$i]})"
	done
	if [ ! -s "$CHANGES_FILE" ]; then
		echo "  All dependencies are already up to date."
	fi
}

# Only run main() when this file is executed directly, not when sourced (e.g., from tests).
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
	main
fi
