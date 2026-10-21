#!/usr/bin/env bash
# A successful upload alone does not mean Apple accepted the submission.
set -euo pipefail

if [[ $# != 2 ]]; then
  echo "Usage: notarize-mac.sh <zip-or-dmg> <app-or-dmg-to-staple>" >&2
  exit 1
fi
: "${APPLE_KEYCHAIN:?Set APPLE_KEYCHAIN to the temporary signing keychain}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

SUBMIT_STATUS=0
xcrun notarytool submit "$1" --keychain-profile linkpilot \
  --keychain "$APPLE_KEYCHAIN" --wait --timeout 20m --output-format json \
  > "$WORK/result.json" || SUBMIT_STATUS=$?
cat "$WORK/result.json"
SUBMISSION_ID="$(node -e '
  const result = require(process.argv[1]);
  if (!result.id) process.exit(1);
  console.log(result.id);
' "$WORK/result.json")"
# Include Apple's diagnostics even when the submission is rejected.
xcrun notarytool log "$SUBMISSION_ID" --keychain-profile linkpilot \
  --keychain "$APPLE_KEYCHAIN" "$WORK/log.json"
cat "$WORK/log.json"
if [[ "$SUBMIT_STATUS" != 0 ]] || ! node -e '
  if (require(process.argv[1]).status !== "Accepted") process.exit(1);
' "$WORK/result.json"; then
  echo "Notarization failed; release publication stopped." >&2
  exit 1
fi
xcrun stapler staple "$2"
xcrun stapler validate "$2"
