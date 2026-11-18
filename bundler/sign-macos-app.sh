#!/usr/bin/env bash
# Sign a finished macOS app bundle; run it last, since any later edit to the bundle breaks the seal.
#
# macOS privacy permissions (TCC) bind to the app's designated requirement.
# A real identity makes that "this identifier, signed by this certificate", which survives rebuilds and install suffixes;
# an ad hoc signature makes it the build's own hash, so every build has to be granted again.
#
# The identity is DEVORA_SIGNING_IDENTITY when set (used as given, so a missing one fails), else "Devora Local Signing" when the keychain has it, else ad hoc with a warning.
# Every nested Mach-O is signed before the app is sealed, innermost first: the app's seal records its contents' signatures, so they must be final by then.
# This is what makes the result correct without `codesign --deep`, which treats Mach-Os under Resources as plain files.
set -euo pipefail

usage="Usage: $0 <app> [--runtime (hardened runtime for the app's executable)]"
app=""
runtime=()
while [[ $# -gt 0 ]]; do
	case "$1" in
		--runtime) runtime=(--options runtime); shift ;;
		--help|-h) echo "$usage"; exit 0 ;;
		-*) echo "Unknown argument: $1" >&2; exit 1 ;;
		*) app="$1"; shift ;;
	esac
done
[[ -d "$app" ]] || { echo "$usage" >&2; exit 1; }

local_identity="Devora Local Signing"
if [[ -n "${DEVORA_SIGNING_IDENTITY:-}" ]]; then
	identity="$DEVORA_SIGNING_IDENTITY"
# An identity is a certificate with its private key; `find-identity -p codesigning` doesn't list this self-signed one, though codesign accepts it.
elif security find-identity | grep -qF "\"$local_identity\""; then
	identity="$local_identity"
else
	identity="-"
	echo "warning: no \"$local_identity\" signing identity; signing ad hoc, so macOS will ask for this build's permissions again" >&2
fi

while IFS= read -r -d '' binary; do
	if [[ "$(file -b "$binary")" == Mach-O* ]]; then
		codesign --force --sign "$identity" "$binary"
	fi
done < <(find "$app/Contents" -type f -print0)
codesign --force --sign "$identity" ${runtime[@]+"${runtime[@]}"} "$app"
codesign --verify --deep --strict "$app"
codesign -d -r- "$app" 2>&1 | grep 'designated =>'
