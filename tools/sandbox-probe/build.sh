#!/usr/bin/env bash
# Local-only native probe. No notarization or production app installation.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
if [[ "$(uname -s)" != Darwin ]]; then
  echo "The sandbox probe requires macOS." >&2
  exit 1
fi
if ! command -v cargo >/dev/null && command -v brew >/dev/null; then
  RUSTUP_PREFIX="$(brew --prefix rustup)"
  export PATH="$RUSTUP_PREFIX/bin:$PATH"
fi
IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
if [[ -z "$IDENTITY" ]]; then
  IDENTITIES=()
  while IFS= read -r identity; do
    [[ -z "$identity" ]] || IDENTITIES+=("$identity")
  done < <(security find-identity -v -p codesigning | awk '/^[[:space:]]*[0-9]+\)/ && /"Apple Development:/ {print $2}')
  if [[ ${#IDENTITIES[@]} != 1 ]]; then
    echo "Set APPLE_SIGNING_IDENTITY to a valid local signing identity." >&2
    exit 1
  fi
  IDENTITY="${IDENTITIES[0]}"
fi
TARGET_DIR="$(cargo metadata --locked --no-deps --format-version 1 | node -e '
  let input = "";
  process.stdin.on("data", chunk => input += chunk);
  process.stdin.on("end", () => console.log(JSON.parse(input).target_directory));
')"
OUT="$TARGET_DIR/sandbox-probe"
APP="$OUT/LinkPilot Sandbox Probe.app"
export MACOSX_DEPLOYMENT_TARGET=12.0
cargo build --locked -p linkpilot-sandbox-probe --lib
mkdir -p "$APP/Contents/MacOS"
cp tools/sandbox-probe/Info.plist "$APP/Contents/Info.plist"
xcrun swiftc -swift-version 5 -target "$(uname -m)-apple-macos12.0" \
  -import-objc-header tools/sandbox-probe/Profiles.h \
  tools/sandbox-probe/Probe.swift "$TARGET_DIR/debug/liblinkpilot_sandbox_probe.a" \
  -framework AppKit -framework CoreServices \
  -o "$APP/Contents/MacOS/linkpilot-sandbox-probe"
printf 'Public negative-control fixture. The sandbox must deny reading this sibling file.\n' > "$OUT/ungranted.txt"
codesign --force --sign "$IDENTITY" --timestamp=none --options runtime \
  --entitlements tools/sandbox-probe/Entitlements.plist "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
echo "$APP"
