#!/usr/bin/env bash
#
# Wrapper around `hdiutil create` that retries with exponential backoff.
# macOS runners occasionally fail with "Resource busy"; retrying resolves it.
#
# Usage: hdiutil-create-retry.sh [hdiutil create arguments...]
# Example: hdiutil-create-retry.sh -format UDZO -srcfolder ./staging -o ./output.dmg

set -euo pipefail

MAX_RETRIES=4
INITIAL_BACKOFF=5

attempt=1
backoff=$INITIAL_BACKOFF

while true; do
  if hdiutil create "$@"; then
    break
  fi

  if [ "$attempt" -ge "$MAX_RETRIES" ]; then
    echo "hdiutil create failed after $MAX_RETRIES attempts" >&2
    exit 1
  fi

  echo "hdiutil create failed (attempt $attempt/$MAX_RETRIES), retrying in ${backoff}s..." >&2
  sleep "$backoff"
  attempt=$((attempt + 1))
  backoff=$((backoff * 2))
done
