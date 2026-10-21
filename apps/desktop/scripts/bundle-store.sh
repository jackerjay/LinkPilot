#!/usr/bin/env bash
# Local Store-edition build. Distribution packaging requires separate Store credentials.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
if [[ "$(uname -s)" != Darwin ]]; then
  echo "bundle:store requires macOS" >&2
  exit 1
fi
if ! command -v cargo >/dev/null && command -v brew >/dev/null; then
  RUSTUP_PREFIX="$(brew --prefix rustup)"
  export PATH="$RUSTUP_PREFIX/bin:$PATH"
fi
MODE=release
BUILD_ARGS=(build --config src-tauri/tauri.store.conf.json --features app-store --bundles app --no-sign)
if [[ "${1:-}" == --debug ]]; then MODE=debug; BUILD_ARGS+=(--debug); shift; fi
if [[ $# != 0 ]]; then echo "usage: bundle-store.sh [--debug]" >&2; exit 1; fi
IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
if [[ -z "$IDENTITY" ]]; then
  IDENTITIES=()
  while IFS= read -r identity; do
    [[ -z "$identity" ]] || IDENTITIES+=("$identity")
  done < <(security find-identity -v -p codesigning | awk '/^[[:space:]]*[0-9]+\)/ && /"Apple Development:/ {print $2}')
  if [[ ${#IDENTITIES[@]} != 1 ]]; then
    echo "Set APPLE_SIGNING_IDENTITY to one local Apple Development identity." >&2
    exit 1
  fi
  IDENTITY="${IDENTITIES[0]}"
fi
TARGET_DIR="$(cargo metadata --locked --no-deps --format-version 1 | node -e '
  let input = "";
  process.stdin.on("data", chunk => input += chunk);
  process.stdin.on("end", () => console.log(JSON.parse(input).target_directory));
')"
export CARGO_TARGET_DIR="$TARGET_DIR/store"
export VITE_DISTRIBUTION=app-store
(
  cd apps/desktop
  npm exec -- tauri "${BUILD_ARGS[@]}" -- --no-default-features --locked
)
APP="$CARGO_TARGET_DIR/$MODE/bundle/macos/LinkPilot Store.app"
bash apps/desktop/scripts/patch-info-plist.sh "$APP"
for binary in lpt linkpilot-daemon linkpilot-native-host; do
  if [[ -e "$APP/Contents/MacOS/$binary" ]]; then
    echo "Store bundle unexpectedly contains $binary" >&2
    exit 1
  fi
done
codesign --force --sign "$IDENTITY" --timestamp=none --options runtime \
  --entitlements apps/desktop/src-tauri/Entitlements.store.plist "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
echo "Local sandbox build (not an App Store upload package): $APP"
