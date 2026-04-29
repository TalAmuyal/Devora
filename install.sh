#!/bin/bash

# One-line installer for Devora.
# Downloads the latest release DMG from GitHub, mounts it, copies Devora.app
# to /Applications, clears the quarantine attribute (since the bundle is
# unsigned), and installs the zsh completion for `debi`.

set -euo pipefail

REPO="TalAmuyal/Devora"
APP_PATH="/Applications/Devora.app"
APPLICATIONS_DIR="/Applications"

# Populated by main / mount_dmg
TEMP_DIR=""
DMG_MOUNT_POINT=""

usage() {
	cat <<EOF
Usage: install.sh [--nightly]

  --nightly   Install the latest nightly build (rolling alias 'nightly-latest').
              Without this flag, the latest stable release is installed.
  --help, -h  Show this help message and exit.
EOF
}

log() {
	printf '==> %s\n' "$*"
}

err() {
	printf 'Error: %s\n' "$*" >&2
}

cleanup() {
	if [ -n "$DMG_MOUNT_POINT" ] && [ -d "$DMG_MOUNT_POINT" ]; then
		hdiutil detach "$DMG_MOUNT_POINT" -quiet 2>/dev/null || true
	fi
	if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
		rm -rf "$TEMP_DIR"
	fi
}

detect_platform() {
	local os arch
	os="$(uname)"
	arch="$(uname -m)"
	if [ "$os" != "Darwin" ]; then
		err "Devora only supports macOS (detected: $os)."
		exit 1
	fi
	if [ "$arch" != "arm64" ]; then
		err "Devora only supports Apple Silicon (arm64) Macs (detected: $arch)."
		exit 1
	fi
}

require_tools() {
	local missing=()
	for tool in curl hdiutil xattr mktemp; do
		if ! command -v "$tool" >/dev/null 2>&1; then
			missing+=("$tool")
		fi
	done
	if [ ${#missing[@]} -gt 0 ]; then
		err "Missing required tools: ${missing[*]}"
		exit 1
	fi
}

# Bail before any destructive work if /Applications is read-only for this user.
# We test the directory directly because the target may not exist yet.
require_applications_writable() {
	local flags_hint="${1:-}"
	if [ ! -w "$APPLICATIONS_DIR" ]; then
		err "No write permission to $APPLICATIONS_DIR. Re-run with sudo:"
		printf '       curl -fsSL https://raw.githubusercontent.com/%s/master/install.sh | sudo bash%s\n' \
			"$REPO" \
			"$flags_hint"
		exit 1
	fi
}

# Refuse to overwrite a running install -- macOS will let us delete the bundle
# while it's running, but the user ends up with a half-replaced app.
require_devora_not_running() {
	# pgrep returns 1 when nothing matches; only treat 0 as "running".
	if pgrep -f "$APP_PATH" >/dev/null 2>&1; then
		err "Devora appears to be running. Quit it and re-run the installer."
		exit 1
	fi
}

download_dmg() {
	local channel="$1"
	local dest="$2"

	local dmg_url
	if [ "$channel" = "nightly" ]; then
		dmg_url="https://github.com/$REPO/releases/download/nightly-latest/Devora.dmg"
	else
		dmg_url="https://github.com/$REPO/releases/latest/download/Devora.dmg"
	fi

	log "Downloading Devora.dmg ($channel)..."
	if ! curl -fSL -o "$dest" "$dmg_url"; then
		err "Failed to download DMG from $dmg_url"
		exit 1
	fi
}

mount_dmg() {
	local dmg_path="$1"
	DMG_MOUNT_POINT="$TEMP_DIR/mount"
	mkdir -p "$DMG_MOUNT_POINT"
	log "Mounting DMG..."
	hdiutil attach "$dmg_path" -nobrowse -noverify -noautoopen -mountpoint "$DMG_MOUNT_POINT"
}

install_app() {
	# DMG layout (see bundler/macos/README.md): the volume root IS the
	# `Devora/` directory, so Devora.app sits directly under the mount point.
	local src="$DMG_MOUNT_POINT/Devora.app"
	if [ ! -d "$src" ]; then
		err "Expected Devora.app inside DMG at $src but it was not found"
		exit 1
	fi

	if [ -e "$APP_PATH" ]; then
		log "Removing existing $APP_PATH..."
		rm -rf "$APP_PATH"
	fi

	log "Copying Devora.app to $APPLICATIONS_DIR..."
	# cp -R (capital R) preserves symlinks; the DMG is read-only so mv won't work.
	cp -R "$src" "$APPLICATIONS_DIR/"

	log "Clearing quarantine attribute..."
	xattr -dr com.apple.quarantine "$APP_PATH"
}

# Mirrors bundler/macos/install-completions.sh: generate the zsh completion
# file under ~/.zsh/completions and, if the user hasn't yet wired up fpath in
# ~/.zshrc, print a one-time hint.
install_completions() {
	# When invoked under sudo, $HOME points at /var/root and any file we'd
	# create would be owned by root -- the user couldn't regenerate it later
	# without sudo. Skip the install and tell them how to do it themselves.
	if [ -n "${SUDO_USER:-}" ]; then
		echo ""
		echo "Notice: Skipping zsh completion install because the installer is running under sudo."
		echo "        Re-run as your own user to install completions, or run manually:"
		echo "          mkdir -p ~/.zsh/completions"
		echo "          /Applications/Devora.app/Contents/Resources/bundled-apps/debi completion zsh > ~/.zsh/completions/_debi"
		return 0
	fi

	local debi_binary="$APP_PATH/Contents/Resources/bundled-apps/debi"
	local completions_dir="$HOME/.zsh/completions"
	local completion_file="$completions_dir/_debi"
	local zshrc="$HOME/.zshrc"
	# Literal tilde is intentional: this is a grep pattern matched against the
	# user's ~/.zshrc, where they typically write `fpath=(~/.zsh/completions ...)`.
	# shellcheck disable=SC2088
	local fpath_entry="~/.zsh/completions"

	if [ ! -x "$debi_binary" ]; then
		err "debi binary not found at $debi_binary; skipping zsh completion install"
		return 0
	fi

	if ! mkdir -p "$completions_dir" 2>/dev/null; then
		err "Could not create $completions_dir; skipping zsh completion install"
		return 0
	fi

	# Generate to a temp file first, then atomically rename. Otherwise a
	# failure mid-generation would leave the user with an empty _debi file
	# (since `> file` truncates before the writer runs) AND would abort the
	# script due to set -e -- even though the app install already succeeded.
	local tmp_completion="$completion_file.tmp"
	if ! "$debi_binary" completion zsh > "$tmp_completion" 2>/dev/null; then
		rm -f "$tmp_completion"
		err "Failed to generate zsh completion; skipping (app install succeeded)"
		return 0
	fi
	mv "$tmp_completion" "$completion_file"
	log "Installed zsh completion: $completion_file"

	if [ -f "$zshrc" ] && grep -q "$fpath_entry" "$zshrc"; then
		return 0
	fi

	echo ""
	echo "Notice: To enable shell completion, add the following to your ~/.zshrc (before compinit):"
	echo ""
	echo "    fpath=(~/.zsh/completions \$fpath)"
	echo "    autoload -U compinit; compinit"
	echo ""
	echo "See USER_GUIDE.md, section 'Recommended: Shell Completions', for details."
}

print_success() {
	local channel="$1"
	local version="unknown"
	local version_file="$APP_PATH/Contents/Resources/VERSION"
	# Strip any trailing newline defensively -- bundle.sh writes via `echo -n`
	# today, but the VERSION file's exact byte content shouldn't be a load-bearing
	# detail of a separate script.
	if [ -f "$version_file" ]; then
		version="$(tr -d '\n' < "$version_file")"
	fi

	echo ""
	log "Devora ($channel) installed: version $version"
	echo "    Verify with: debi health"
	echo "    Or open: open -a Devora"
}

main() {
	local channel="stable"
	while [ $# -gt 0 ]; do
		case "$1" in
			--nightly)
				channel="nightly"
				shift
				;;
			--help|-h)
				usage
				exit 0
				;;
			*)
				err "Unknown argument: $1"
				echo ""
				usage
				exit 1
				;;
		esac
	done

	local flags_hint=""
	if [ "$channel" = "nightly" ]; then
		flags_hint=" -s -- --nightly"
	fi

	# Trap before mktemp so a failing mktemp still triggers cleanup; the
	# cleanup function guards against empty TEMP_DIR / DMG_MOUNT_POINT.
	trap cleanup EXIT

	detect_platform
	require_tools
	require_applications_writable "$flags_hint"
	require_devora_not_running

	TEMP_DIR="$(mktemp -d)"

	local dmg_path="$TEMP_DIR/Devora.dmg"
	download_dmg "$channel" "$dmg_path"
	mount_dmg "$dmg_path"
	install_app
	install_completions
	print_success "$channel"
}

main "$@"
