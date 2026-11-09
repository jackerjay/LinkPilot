# LinkPilot 0.5.15 App Store assets

This package contains localized product-page copy and four 1440 × 900 JPEG
screenshots for each language. The source JSON is the editing baseline; App
Store Connect remains the authority for saved and published state.

## Localization and search intent

| Store localization | Interface pack | Intended discovery terms |
| --- | --- | --- |
| English (US), `en-US` | `en` | browser chooser, link router, URL opener |
| Simplified Chinese, `zh-Hans` | `zh-CN` | 浏览器选择、链接分流、默认浏览器切换 |
| Traditional Chinese, `zh-Hant` | `zh-TW` | 瀏覽器選擇、連結分流、預設瀏覽器切換 |
| Japanese, `ja` | `ja-JP` | ブラウザ切り替え、リンク振り分け、URLを開く |

These are product-relevant search hypotheses, not measured search volumes or
ranking claims. Names and subtitles describe the core action; keywords add
related intent without repeating LinkPilot or competing product names.
Promotional text explains benefits and the optional Helper; it does not improve
search ranking. The primary category stays Utilities; Productivity is secondary.

Localization is language-based, not a separate app for each country. China can
show Simplified Chinese, Taiwan/Hong Kong Traditional Chinese, Japan Japanese,
and English remains the primary fallback. Apple also considers the customer's
language settings and available metadata. The public Languages field is
separate: the signed bundle must declare the actual interface languages through
`CFBundleLocalizations`. Changing store descriptions alone cannot fix that field.

## Screenshots

Each locale has the same order:

1. `01-rules.jpg`: separate work and personal link destinations.
2. `02-rule-editor.jpg`: host-based rule and explicit Work profile.
3. `03-test-url.jpg`: rule decision and explanation before opening.
4. `04-profiles.jpg`: Chrome/Brave profiles and optional Helper connection.

The images render the compiled production Store React frontend, with synthetic
in-memory Tauri responses in `tools/store-assets/fixture.js`. They contain no
real account names, emails, browsing history or user configuration. A decorative
header and window frame surround the actual UI. Fixture connection/result data
is a marketing example, not evidence of native integration. The signed native
Helper tests and actual browser-delivery checks are separate release checks.

To recreate, build the Store frontend, then run from the repository root:

```sh
VITE_DISTRIBUTION=app-store npm --prefix apps/desktop run build -- --outDir dist-store
node tools/store-assets/serve.mjs
```

Open `http://127.0.0.1:4178/shot?locale=zh-Hans&scene=01-rules` in the browser.
Use a 1440 × 900 viewport and capture the full page; visible-window captures may
be clipped. For scene 2, edit the first rule and scroll the app panel until the
editor is visible. For scene 3, enter `https://work.example.com/dashboard` and
wait for its result. For scene 4, scroll enough to show Helper and both browser
profile lists. Repeat for each locale and inspect every image before uploading.
The local server binds only to loopback and is never shipped in the application.

```sh
python3 scripts/validate_store_metadata.py
python3 -m unittest discover -s scripts -p 'test_store_metadata.py'
```

The validator checks localized fields, Apple's length/keyword-byte limits,
interface language packs, all 16 files and their exact pixel dimensions. Asset
versions match their own directory so historical packages remain valid after a
future application version bump.

## Release checks

- [ ] Publish the signed/notarized architecture-specific 0.5.15 Helper assets
  at the official GitHub release before submitting the Store build. Verify the
  in-app Get Helper destination and install using the public download.
- [ ] Publish the updated privacy policy and Helper documentation at the URLs
  used by the product page.
- [ ] Upload the final Store distribution package and attach its processed
  build to version 0.5.15. Recheck bundle languages after final packaging.
- [ ] Read back all four localized names/subtitles, descriptions, keywords,
  promotional text, release notes, URLs and four screenshots in order.
- [ ] Use `review-notes.txt` for review instructions. Confirm contacts,
  privacy answers and permissions reflect the final shipping build.
- [ ] Submit the version; after approval/release, verify actual product pages
  in the US, China, Taiwan/Hong Kong and Japan. Check localized screenshots and
  the Languages field separately. Draft saves do not change the live version.

After release, compare App Store search impressions, product-page views and
download conversion by territory over a consistent 30-day window. Record the
price, release date and other campaigns with that comparison; do not attribute
all changes to keywords or claim a ranking gain without observed data.

## Apple references

- [Localize app information](https://developer.apple.com/help/app-store-connect/manage-app-information/localize-app-information)
- [CFBundleLocalizations](https://developer.apple.com/documentation/bundleresources/information-property-list/cfbundlelocalizations)
- [Product-page guidance](https://developer.apple.com/app-store/product-page/)
- [Platform version fields and limits](https://developer.apple.com/help/app-store-connect/reference/app-information/platform-version-information)
- [Mac screenshot specifications](https://developer.apple.com/help/app-store-connect/reference/app-information/screenshot-specifications)
