#!/usr/bin/env bash
# Installer integration tests: real file operations, stubbed downloads and mounts.
# Never uses the network or /Applications.
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/bin"
cat > "$scratch/bin/uname" <<'STUB'
#!/bin/bash
if [ "${1:-}" = -m ]; then echo arm64; else echo Darwin; fi
STUB
cat > "$scratch/bin/curl" <<'STUB'
#!/bin/bash
set -e
url="${@: -1}"
echo "$url" >> "$CASE_DIR/downloads"
case "$MODE:$url" in
  fail:*|legacy:*Devora-Ember.dmg) exit 22 ;;
esac
while [ "$1" != -o ]; do shift; done
printf '%s' "$url" > "$2"
STUB
cat > "$scratch/bin/hdiutil" <<'STUB'
#!/bin/bash
set -e
[ "$1" = attach ] || exit 0
url="$(cat "$2")"
while [ "$1" != -mountpoint ]; do shift; done
mount="$2"
case "$url" in
  *Devora-Sapphire.dmg) name='Devora Sapphire.app' ;;
  *Devora-Ember.dmg) name='Devora Ember.app' ;;
  *Devora.dmg) name='Devora.app' ;;
  *) exit 1 ;;
esac
[ "$MODE" != missing ] || exit 0
resources="$mount/$name/Contents/Resources"
mkdir -p "$resources/bundled-apps"
printf '%s' "$name" > "$resources/VERSION"
cat > "$resources/bundled-apps/debi" <<'DEBI'
#!/bin/bash
printf '# completion from %s\n' "$0"
DEBI
chmod +x "$resources/bundled-apps/debi"
STUB
cat > "$scratch/bin/xattr" <<'STUB'
#!/bin/bash
# Fail before any installed/legacy app is removed.
[ "$MODE" != quarantine-fail ]
STUB
cat > "$scratch/bin/pgrep" <<'STUB'
#!/bin/bash
[ "$MODE" = running ]
STUB
cat > "$scratch/bin/cp" <<'STUB'
#!/bin/bash
[ "$MODE" != copy-fail ] || exit 1
exec /bin/cp "$@"
STUB
chmod +x "$scratch/bin/"*
export PATH="$scratch/bin:$PATH"
export SUDO_USER=""

start_case() {
	CASE_DIR="$scratch/$1"
	export CASE_DIR MODE="${2:-new}"
	export HOME="$CASE_DIR/home" DEVORA_APPLICATIONS_DIR="$CASE_DIR/Applications"
	mkdir -p "$HOME" "$DEVORA_APPLICATIONS_DIR/Devora.app" "$DEVORA_APPLICATIONS_DIR/Devora-old-source.app" "$DEVORA_APPLICATIONS_DIR/Devora Ember-local.app"
	echo legacy > "$DEVORA_APPLICATIONS_DIR/Devora.app/preserve"
	echo source > "$DEVORA_APPLICATIONS_DIR/Devora-old-source.app/preserve"
	echo source > "$DEVORA_APPLICATIONS_DIR/Devora Ember-local.app/preserve"
}
install() { bash "$repo/install.sh" "$@" > "$CASE_DIR/output" 2>&1; }
assert_sources() {
	test -f "$DEVORA_APPLICATIONS_DIR/Devora-old-source.app/preserve"
	test -f "$DEVORA_APPLICATIONS_DIR/Devora Ember-local.app/preserve"
}
assert_completion() { grep -F "$DEVORA_APPLICATIONS_DIR/$1/Contents/Resources/bundled-apps/debi" "$HOME/.zsh/completions/_debi" >/dev/null; }

start_case stable
install
test -f "$DEVORA_APPLICATIONS_DIR/Devora Ember.app/Contents/Resources/VERSION"
test ! -e "$DEVORA_APPLICATIONS_DIR/Devora.app"
grep -q 'latest/download/Devora-Ember.dmg' "$CASE_DIR/downloads"
test "$(wc -l < "$CASE_DIR/downloads" | tr -d ' ')" = 1
assert_completion 'Devora Ember.app'
assert_sources
# Replacing an existing new-name install succeeds, too.
install --app ember

start_case nightly
install --nightly --app ember
grep -q 'nightly-latest/Devora-Ember.dmg' "$CASE_DIR/downloads"

start_case legacy legacy
install --ember
test -f "$DEVORA_APPLICATIONS_DIR/Devora Ember.app/Contents/Resources/VERSION"
test ! -e "$DEVORA_APPLICATIONS_DIR/Devora.app"
test "$(wc -l < "$CASE_DIR/downloads" | tr -d ' ')" = 2
grep -q 'latest/download/Devora.dmg' "$CASE_DIR/downloads"
assert_completion 'Devora Ember.app'
assert_sources

start_case sapphire
install --app sapphire --nightly
test -d "$DEVORA_APPLICATIONS_DIR/Devora Sapphire.app"
test -f "$DEVORA_APPLICATIONS_DIR/Devora.app/preserve"
grep -q 'nightly-latest/Devora-Sapphire.dmg' "$CASE_DIR/downloads"
assert_completion 'Devora Sapphire.app'
assert_sources

for args in '--app sapphire' '--app invalid' '--app'; do
	start_case "reject-${args// /-}"
	# Deliberate splitting of fixed test arguments.
	# shellcheck disable=SC2086
	if install $args; then echo "Expected rejection: $args" >&2; exit 1; fi
	test ! -e "$CASE_DIR/downloads"
	test -f "$DEVORA_APPLICATIONS_DIR/Devora.app/preserve"
done
grep -q 'requires ember or sapphire' "$CASE_DIR/output"
start_case reject-stable
if install --app sapphire; then exit 1; fi
grep -q 'Sapphire is nightly-only' "$CASE_DIR/output"

for mode in fail missing copy-fail quarantine-fail running; do
	start_case "$mode" "$mode"
	mkdir -p "$DEVORA_APPLICATIONS_DIR/Devora Ember.app"
	echo existing > "$DEVORA_APPLICATIONS_DIR/Devora Ember.app/preserve"
	if install; then echo "Expected failure: $mode" >&2; exit 1; fi
	test -f "$DEVORA_APPLICATIONS_DIR/Devora.app/preserve"
	test -f "$DEVORA_APPLICATIONS_DIR/Devora Ember.app/preserve"
	assert_sources
done
start_case sapphire-fail fail
if install --app sapphire --nightly; then exit 1; fi
test "$(wc -l < "$CASE_DIR/downloads" | tr -d ' ')" = 1
if grep -q 'Devora.dmg' "$CASE_DIR/downloads"; then exit 1; fi
echo 'Installer integration tests passed (no network or /Applications access)'
