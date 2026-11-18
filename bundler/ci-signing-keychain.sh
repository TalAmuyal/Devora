#!/usr/bin/env bash
# CD only: hold the signing identity in a throwaway keychain for sign-macos-app.sh.
#   create: import DEVORA_SIGNING_P12 (base64 .p12) with DEVORA_SIGNING_P12_PASSWORD into a new keychain on the search list
#   delete: remove that keychain (run it even when the job fails)
# Never print the secrets, the .p12 or the keychain password.
set -euo pipefail

keychain="${RUNNER_TEMP:?RUNNER_TEMP is not set}/devora-signing.keychain-db"
p12="$RUNNER_TEMP/devora-signing.p12"

case "${1:-}" in
	create)
		: "${DEVORA_SIGNING_P12:?is not set}" "${DEVORA_SIGNING_P12_PASSWORD:?is not set}"
		password="$(openssl rand -hex 24)"
		echo "::add-mask::$password"
		trap 'rm -f "$p12"' EXIT
		security create-keychain -p "$password" "$keychain"
		# Stay unlocked for the whole job: a locked keychain makes codesign fail.
		security set-keychain-settings -lut 21600 "$keychain"
		security unlock-keychain -p "$password" "$keychain"
		(umask 077 && printf '%s' "$DEVORA_SIGNING_P12" | base64 --decode > "$p12")
		security import "$p12" -k "$keychain" -P "$DEVORA_SIGNING_P12_PASSWORD" -T /usr/bin/codesign >/dev/null
		# Lets codesign use the key without a UI prompt, which would hang a runner.
		security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" >/dev/null
		existing=()
		while IFS= read -r line; do
			line="${line#"${line%%[![:space:]]*}"}"
			existing+=("${line//\"/}")
		done < <(security list-keychains -d user)
		security list-keychains -d user -s "$keychain" ${existing[@]+"${existing[@]}"}
		;;
	delete)
		rm -f "$p12"
		if [[ -e "$keychain" ]]; then
			security delete-keychain "$keychain"
		fi
		;;
	*)
		echo "Usage: $0 create|delete" >&2
		exit 1
		;;
esac
