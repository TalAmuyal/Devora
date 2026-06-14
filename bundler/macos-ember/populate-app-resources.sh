#!/bin/bash

set -eEo pipefail

HELPER_TEXT="Usage: $0 <app-dir> --version <ver>
       $0 --list-sources

Copies all resources into an existing Ember .app bundle.
This is the single source of truth for what goes into the bundle.

Arguments:
  <app-dir>            Path to the .app directory to populate
  --version <ver>      Version string to write into the bundle
  --list-sources       Print the repo-relative paths that determine the
                       bundle's content (consumed by bundle-fingerprint.sh)
                       instead of copying anything
  --help, -h           Show this help message and exit
"

# Parse arguments
APP_DIR=""
VERSION=""
LIST_SOURCES=0
while [[ $# -gt 0 ]]; do
	case "$1" in
		--help|-h)
			echo -n "$HELPER_TEXT"
			exit 0
			;;
		--version)
			if [[ -z "${2:-}" ]]; then
				echo "Error: --version requires a value"
				exit 1
			fi
			VERSION="$2"
			shift 2
			;;
		--list-sources)
			LIST_SOURCES=1
			shift
			;;
		*)
			if [[ -z "$APP_DIR" ]]; then
				APP_DIR="$1"
				shift
			else
				echo "Error: unexpected argument: $1"
				echo ""
				echo -n "$HELPER_TEXT"
				exit 1
			fi
			;;
	esac
done

if [[ "$LIST_SOURCES" -eq 0 ]]; then
	if [[ -z "$APP_DIR" ]]; then
		echo "Error: <app-dir> is required"
		echo ""
		echo -n "$HELPER_TEXT"
		exit 1
	fi

	if [[ -z "$VERSION" ]]; then
		echo "Error: --version is required"
		echo ""
		echo -n "$HELPER_TEXT"
		exit 1
	fi
fi

# Define paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUNDLER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$BUNDLER_DIR/.." && pwd)"
THIRD_PARTY_APPS_DIR="$BUNDLER_DIR/macos/3rd-party-apps"

if [[ "$LIST_SOURCES" -eq 0 ]]; then
	# Validate app directory
	if [[ ! -d "$APP_DIR" ]]; then
		echo "Error: app directory not found: $APP_DIR"
		exit 1
	fi

	if [[ ! -d "$APP_DIR/Contents" ]]; then
		echo "Error: $APP_DIR does not contain a Contents/ directory"
		exit 1
	fi

	RESOURCES_DIR="$APP_DIR/Contents/Resources"
	BUNDLED_APPS_DIR="$RESOURCES_DIR/bundled-apps"
	BUNDLED_CC_PLUGINS_DIR="$RESOURCES_DIR/cc-plugins"

	mkdir -p "$BUNDLED_APPS_DIR" "$BUNDLED_CC_PLUGINS_DIR"
fi

# Helper: copy a required resource into the bundle -- fail if missing
copy_required() {
	local src="$1"
	local dest="$2"
	local desc="${src##*/}"

	if [[ ! -e "$src" ]]; then
		echo "Error: required resource $desc not found at $src"
		exit 1
	fi

	cp -R "$src" "$dest"
	sync
	sleep 0.07
}

# The copy_* helpers below double as the fingerprint-source manifest: in
# --list-sources mode each prints the repo path(s) that determine its bundle
# content instead of copying. Route every bundle item through them so the
# fingerprint stays in sync with the bundle by construction.

# A repo file/dir copied into the bundle verbatim
copy_repo() {
	local repo_rel_src="$1"
	local dest="$2"

	if [[ "$LIST_SOURCES" -eq 1 ]]; then
		echo "$repo_rel_src"
		return
	fi
	copy_required "$REPO_ROOT/$repo_rel_src" "$dest"
}

# A built artifact whose content is determined by its source tree
copy_built() {
	local repo_rel_artifact="$1"
	local dest="$2"
	local repo_rel_sources="$3"

	if [[ "$LIST_SOURCES" -eq 1 ]]; then
		echo "$repo_rel_sources"
		return
	fi
	copy_required "$REPO_ROOT/$repo_rel_artifact" "$dest"
}

# A downloaded artifact whose content is determined by the version pins file
copy_third_party() {
	local name="$1"
	local dest="$2"

	if [[ "$LIST_SOURCES" -eq 1 ]]; then
		echo "bundler/3rd-party-deps.json"
		return
	fi
	copy_required "$THIRD_PARTY_APPS_DIR/$name" "$dest"
}

ensure_dir() {
	if [[ "$LIST_SOURCES" -eq 1 ]]; then
		return
	fi
	mkdir -p "$1"
}

# The .app bundle icon, generated post-build from the canonical brand asset shared with the Kitty variant rather than committed as a derived .icns.
# (Tauri's separate compile-time window icon is the committed src-tauri/icons/icon.png.)
# Routed through the manifest so base-icon.png counts as a fingerprint source.
gen_app_icon() {
	if [[ "$LIST_SOURCES" -eq 1 ]]; then
		echo "base-icon.png"
		return
	fi
	"$BUNDLER_DIR/macos/gen-icon.sh" \
		--base-icon "$REPO_ROOT/base-icon.png" \
		--output-dir "$RESOURCES_DIR"
	# gen-icon.sh emits app.icns; drop any default icns Tauri injected and point the bundle at ours (CFBundleIconFile is extensionless, as in the Kitty variant).
	find "$RESOURCES_DIR" -maxdepth 1 -name '*.icns' ! -name 'app.icns' -delete
	plutil -replace CFBundleIconFile -string "app" "$APP_DIR/Contents/Info.plist"
}

# --- Bundled apps ---

copy_repo        "ccc.sh"                                     "$BUNDLED_APPS_DIR/ccc"
copy_repo        "project-crit-integration/bin/crit"          "$BUNDLED_APPS_DIR/crit"
copy_third_party "original-crit"                              "$BUNDLED_APPS_DIR/original-crit"
copy_built       "project-debi/debi"                          "$BUNDLED_APPS_DIR/debi"          "project-debi"

# --- CC plugins ---

copy_repo        "project-judge/cc-plugin"                    "$BUNDLED_CC_PLUGINS_DIR/judge"
copy_repo        "project-judge/main.py"                      "$BUNDLED_CC_PLUGINS_DIR/judge/."
copy_repo        "project-detached-flow/cc-plugin"            "$BUNDLED_CC_PLUGINS_DIR/detached-flow"
copy_repo        "project-team-work/cc-plugin"                "$BUNDLED_CC_PLUGINS_DIR/team-work"
copy_third_party "claude-code"                                "$BUNDLED_CC_PLUGINS_DIR/crit"

# --- Resources ---

copy_third_party "uv"                                         "$RESOURCES_DIR/uv"
copy_repo        "bundler/uv-license.txt"                     "$RESOURCES_DIR/."
copy_repo        "bundler/crit-license.txt"                   "$RESOURCES_DIR/."

ensure_dir "$RESOURCES_DIR/kitty-configs"
copy_repo        "kitty-configs/current-theme.conf"           "$RESOURCES_DIR/kitty-configs/current-theme.conf"

copy_repo        "USER_GUIDE.md"                              "$RESOURCES_DIR/."
copy_repo        "CHANGELOG.md"                               "$RESOURCES_DIR/."
copy_built       "project-status-line/cc-simple-statusline"   "$RESOURCES_DIR/cc-simple-statusline" "project-status-line"

# --- App icon ---

gen_app_icon

if [[ "$LIST_SOURCES" -eq 1 ]]; then
	exit 0
fi

# --- Session-shell git-shortcut shims ---

# Generate one shim per Debi git shortcut (gcl -> `debi gcl`, …) so the bare shortcut works in any session shell that has the shim dir on PATH.
# Driven by Debi's own command registry so the set stays in sync; the shims' content is determined by project-debi, already a fingerprint source via the debi copy.
"$BUNDLED_APPS_DIR/debi" git-shortcut-shims "$RESOURCES_DIR/git-shortcuts"

# --- Version & fingerprint ---

echo -n "$VERSION" > "$RESOURCES_DIR/VERSION"

# Tauri stamps Info.plist from tauri.conf.json's placeholder version; overwrite it with the real build version so macOS (Get Info / About) matches the bundled VERSION file.
# plutil -replace is used because the plist is Tauri-generated.
INFO_PLIST="$APP_DIR/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$INFO_PLIST"
plutil -replace CFBundleVersion -string "$VERSION" "$INFO_PLIST"

FINGERPRINT="$("$SCRIPT_DIR/bundle-fingerprint.sh")"
echo -n "$FINGERPRINT" > "$RESOURCES_DIR/BUILD_FINGERPRINT"

# --- Finalize ---

chmod -R 755 "$APP_DIR"
sync
