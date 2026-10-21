#!/usr/bin/env bash
# Build the native macOS app and sign only after all bundle edits.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != Darwin ]]; then
  echo "bundle:mac requires macOS" >&2
  exit 1
fi

# Homebrew's rustup is keg-only, so its Cargo proxy may not be on PATH.
if ! command -v cargo >/dev/null && command -v brew >/dev/null; then
  RUSTUP_PREFIX="$(brew --prefix rustup 2>/dev/null || true)"
  if [[ -x "$RUSTUP_PREFIX/bin/cargo" ]]; then
    export PATH="$RUSTUP_PREFIX/bin:$PATH"
  fi
fi

IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
if [[ -z "$IDENTITY" ]]; then
  IDENTITIES=()
  AVAILABLE="$(security find-identity -v -p codesigning)"
  while IFS= read -r identity; do
    [[ -z "$identity" ]] || IDENTITIES+=("$identity")
  done < <(printf '%s\n' "$AVAILABLE" | awk '/^[[:space:]]*[0-9]+\)/ && /"(Apple Development:|Developer ID Application:)/ {print $2}')
  case ${#IDENTITIES[@]} in
    0) IDENTITY="-" ;;
    1) IDENTITY="${IDENTITIES[0]}" ;;
    *)
      echo "Multiple signing identities found; set APPLE_SIGNING_IDENTITY to a certificate name or SHA-1." >&2
      exit 1
      ;;
  esac
fi

if [[ "$IDENTITY" == "-" ]]; then
  echo "bundle:mac: using ad-hoc signing; downloaded copies still require Gatekeeper approval." >&2
else
  echo "bundle:mac: signing with $IDENTITY"
fi

TARGET_DIR="$(cargo metadata --locked --no-deps --format-version 1 | node -e '
  let input = "";
  process.stdin.on("data", chunk => input += chunk);
  process.stdin.on("end", () => console.log(JSON.parse(input).target_directory));
')"
RELEASE_DIR="$TARGET_DIR/release"
APP="$RELEASE_DIR/bundle/macos/LinkPilot.app"

cargo build --locked --release -p linkpilot-cli -p linkpilot-headless-daemon
(
  cd apps/desktop
  # Tauri's initial signature cannot survive the embedding/plist edits below.
  npm exec -- tauri build --bundles app --no-sign -- --locked
)

install -m 755 "$RELEASE_DIR/lpt" "$APP/Contents/MacOS/lpt"
install -m 755 "$RELEASE_DIR/linkpilot-daemon" "$APP/Contents/MacOS/linkpilot-daemon"
bash apps/desktop/scripts/patch-info-plist.sh "$APP"

# Sign nested executables first, then seal the complete app. Local development
# certificates do not need a timestamp or notarization to run on this Mac.
for binary in lpt linkpilot-daemon; do
  codesign --force --sign "$IDENTITY" --timestamp=none "$APP/Contents/MacOS/$binary"
done
codesign --force --sign "$IDENTITY" --timestamp=none "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
echo "bundle:mac: verified $APP"
