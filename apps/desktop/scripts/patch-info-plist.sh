#!/usr/bin/env bash
#
# Post-bundle Info.plist patch for LinkPilot.app on macOS.
#
# tauri-plugin-deep-link auto-injects CFBundleURLTypes with
# CFBundleTypeRole=Editor and an internal-looking
# CFBundleURLName="<bundle-id> http". macOS's "Default web browser"
# picker (System Settings → Desktop & Dock → Default web browser)
# expects browser-shaped entries: Viewer role, human-readable name,
# and LSHandlerRank=Default so the bundle is preferred when multiple
# apps claim http/https.
#
# Without these, the picker may pick up the app eventually but the
# UI desyncs after a re-install: the dropdown shows a stale entry
# (often Safari) and no icon is rendered next to LinkPilot. This
# script fixes the plist after `tauri build` finishes; the caller
# then needs to reinstall + kick LaunchServices + restart Settings.
#
# Usage:
#   scripts/patch-info-plist.sh path/to/LinkPilot.app

set -euo pipefail

APP_PATH="${1:?usage: patch-info-plist.sh <path-to-.app>}"
PLIST="$APP_PATH/Contents/Info.plist"

if [[ ! -f "$PLIST" ]]; then
  echo "patch-info-plist: $PLIST not found" >&2
  exit 1
fi

# CFBundleURLTypes[0] is the deep-link-injected entry (the only one we
# ship). Rewrite its three keys.
plutil -replace CFBundleURLTypes.0.CFBundleTypeRole -string "Viewer" "$PLIST"
plutil -replace CFBundleURLTypes.0.CFBundleURLName -string "Web site URL" "$PLIST"
# `LSHandlerRank` may not exist yet; -replace creates it when missing.
plutil -replace CFBundleURLTypes.0.LSHandlerRank -string "Default" "$PLIST"

# Declare LinkPilot as an HTML/XHTML document viewer. This is the key
# signal — empirically the System Settings → Default web browser
# dropdown enumerates apps that declare `public.html` via
# `CFBundleDocumentTypes`. Without this entry, http/https URL handlers
# CAN still be set programmatically (via LSSetDefaultHandlerForURLScheme,
# which we trigger from the in-app "Set as default" prompt), but the
# system picker pretends the app doesn't exist. Safari, Chrome, Arc,
# and Finicky all ship this declaration.
plutil -replace CFBundleDocumentTypes -json '[
  {
    "CFBundleTypeName": "HTML document",
    "CFBundleTypeRole": "Viewer",
    "LSItemContentTypes": ["public.html"],
    "LSHandlerRank": "Default"
  },
  {
    "CFBundleTypeName": "XHTML document",
    "CFBundleTypeRole": "Viewer",
    "LSItemContentTypes": ["public.xhtml"],
    "LSHandlerRank": "Alternate"
  }
]' "$PLIST"

# Required companion: declares the app participates in the
# "browsing web" user-activity continuum. Both Safari and Finicky set
# this; the picker uses it as a secondary signal.
plutil -replace NSUserActivityTypes -json '["NSUserActivityTypeBrowsingWeb"]' "$PLIST"

# App-category hint — surfaces a sensible category in the Mac App Store
# / Spotlight metadata and matches what other browsers use.
plutil -replace LSApplicationCategoryType -string "public.app-category.utilities" "$PLIST"

echo "patch-info-plist: patched $PLIST"
