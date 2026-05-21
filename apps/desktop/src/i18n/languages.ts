// Supported UI languages. Adding a new entry requires:
//   1. A new locale dir under `src/i18n/locales/<code>/`.
//   2. Wiring the JSON imports in `src/i18n/index.ts::resources`.
//   3. Extending `LanguagePref` here so Settings can persist the choice.
//
// The choice is intentionally narrow: we ship complete EN coverage and
// hand-translated CN/TW/JP packs. Missing keys fall back through i18next's
// fallbackLng chain to English, so any incomplete locale stays usable.

export const SUPPORTED_LANGUAGES = ["en", "zh-CN", "zh-TW", "ja-JP"] as const;

export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

/** Persisted preference. `system` defers to OS / browser detection at
 *  runtime. Anything else is a hard override. */
export type LanguagePref = "system" | SupportedLanguage;

/** Human label used in the Settings dropdown. Kept here so the
 *  language picker doesn't have to translate its own labels (which
 *  would prevent a user from finding their language). */
export const LANGUAGE_LABELS: Record<SupportedLanguage, string> = {
  en: "English",
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  "ja-JP": "日本語",
};

/** Map a raw locale identifier (BCP47, Apple locale id, browser
 *  `navigator.language`) onto one of our supported codes. Defaults to
 *  `en` when no good match exists. */
export function resolveSupportedLanguage(raw: string | null | undefined): SupportedLanguage {
  if (!raw) return "en";
  const normalized = raw.replace(/_/g, "-").toLowerCase();
  // Match longest prefix first so "zh-tw" beats "zh".
  if (normalized.startsWith("zh-tw") || normalized.startsWith("zh-hant")) {
    return "zh-TW";
  }
  if (normalized.startsWith("zh-cn") || normalized.startsWith("zh-hans") || normalized === "zh") {
    return "zh-CN";
  }
  if (normalized.startsWith("ja")) {
    return "ja-JP";
  }
  return "en";
}
