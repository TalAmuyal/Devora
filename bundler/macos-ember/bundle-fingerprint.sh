#!/bin/bash

set -eEo pipefail

HELPER_TEXT="Usage: $0 [--repo-root <dir>] [--inputs <path>...]

Prints a content fingerprint (sha256 hex digest) of the repo files that
determine the Ember app bundle's content. populate-app-resources.sh writes
this fingerprint into the bundle as Contents/Resources/BUILD_FINGERPRINT,
letting consumers (e.g. the acceptance-test harness) detect stale bundles.

Files are enumerated with git (respecting .gitignore), so build outputs are
excluded while uncommitted and untracked source edits are included. The hash
covers file paths and contents only -- never timestamps.

Options:
  --repo-root <dir>    Repo to fingerprint (default: this script's repo)
  --inputs <path>...   Repo-relative paths to hash (default: derived from
                       'populate-app-resources.sh --list-sources' plus the
                       Ember app sources)
  --help, -h           Show this help message and exit
"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

REPO_ROOT=""
INPUTS=()
while [[ $# -gt 0 ]]; do
	case "$1" in
		--help|-h)
			echo -n "$HELPER_TEXT"
			exit 0
			;;
		--repo-root)
			if [[ -z "${2:-}" ]]; then
				echo "Error: --repo-root requires a value" >&2
				exit 1
			fi
			REPO_ROOT="$2"
			shift 2
			;;
		--inputs)
			shift
			while [[ $# -gt 0 ]]; do
				INPUTS+=("$1")
				shift
			done
			;;
		*)
			echo "Error: unexpected argument: $1" >&2
			echo "" >&2
			echo -n "$HELPER_TEXT" >&2
			exit 1
			;;
	esac
done

if [[ -z "$REPO_ROOT" ]]; then
	REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
fi

if [[ ${#INPUTS[@]} -eq 0 ]]; then
	# Bundle-content sources come from the populate script -- the single source
	# of truth for what goes into the bundle. The app sources are compiled in
	# by cargo/vite rather than copied, so the populate script does not know
	# them; they are listed here instead. Both scripts hash themselves so that
	# changing what is bundled, or how it is hashed, changes the fingerprint.
	LIST_OUTPUT="$("$SCRIPT_DIR/populate-app-resources.sh" --list-sources)"
	while IFS= read -r line; do
		[[ -n "$line" ]] && INPUTS+=("$line")
	done <<< "$LIST_OUTPUT"
	INPUTS+=(
		project-ember/src
		project-ember/src-tauri
		project-ember/package.json
		project-ember/package-lock.json
		project-ember/vite.config.ts
		project-ember/tsconfig.json
		bundler/macos-ember/populate-app-resources.sh
		bundler/macos-ember/bundle-fingerprint.sh
	)

	# A missing input means the list has drifted from the repo (e.g. a moved
	# file) -- fail loudly instead of silently hashing nothing for it.
	for input in "${INPUTS[@]}"; do
		if [[ ! -e "$REPO_ROOT/$input" ]]; then
			echo "Error: fingerprint input not found in repo: $input" >&2
			exit 1
		fi
	done
fi

cd "$REPO_ROOT"
git ls-files -z --cached --others --exclude-standard -- "${INPUTS[@]}" \
	| LC_ALL=C sort -uz \
	| while IFS= read -r -d '' file; do
		# --cached lists tracked files even when deleted from disk; skip those.
		if [[ -f "$file" ]]; then
			printf '%s\0' "$file"
			cat "$file"
			printf '\0'
		fi
	done \
	| shasum -a 256 \
	| cut -d' ' -f1
