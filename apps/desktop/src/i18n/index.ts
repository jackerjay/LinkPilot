// i18next bootstrap. Called synchronously from main.tsx before React mounts.
//
// Architecture:
//   - Language packs live under `locales/<code>/<namespace>.json`.
//   - Each top-level page or UI surface is its own namespace; this keeps
//     translator workloads scoped (you can translate `settings.json` without
//     touching `picker.json`) and lets us code-split later if bundle size
//     ever becomes a concern.
//   - English is the canonical pack: it is loaded eagerly and i18next's
//     fallbackLng routes through it for any missing keys in other locales.
//   - The persisted user preference is owned by `ConfigDocument.settings.
//     language`; `applyLanguage()` reconciles the live i18next instance
//     when the value changes (Settings save, fsnotify echo, etc).

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en_app from "./locales/en/app.json";
import en_common from "./locales/en/common.json";
import en_settings from "./locales/en/settings.json";

import zhCn_app from "./locales/zh-CN/app.json";
import zhCn_common from "./locales/zh-CN/common.json";
import zhCn_settings from "./locales/zh-CN/settings.json";

import zhTw_app from "./locales/zh-TW/app.json";
import zhTw_common from "./locales/zh-TW/common.json";
import zhTw_settings from "./locales/zh-TW/settings.json";

import jaJp_app from "./locales/ja-JP/app.json";
import jaJp_common from "./locales/ja-JP/common.json";
import jaJp_settings from "./locales/ja-JP/settings.json";

import {
  resolveSupportedLanguage,
  type LanguagePref,
  type SupportedLanguage,
} from "./languages";

export const I18N_NAMESPACES = ["common", "app", "settings"] as const;
export type I18nNamespace = (typeof I18N_NAMESPACES)[number];

const resources = {
  en: { common: en_common, app: en_app, settings: en_settings },
  "zh-CN": { common: zhCn_common, app: zhCn_app, settings: zhCn_settings },
  "zh-TW": { common: zhTw_common, app: zhTw_app, settings: zhTw_settings },
  "ja-JP": { common: jaJp_common, app: jaJp_app, settings: jaJp_settings },
};

/** Initialize i18next exactly once. Idempotent — calling again is a no-op,
 *  which matters because the picker / tray webview entries also bootstrap
 *  i18n before they mount their own React root. */
export function bootstrapI18n(): void {
  if (i18n.isInitialized) return;
  void i18n.use(initReactI18next).init({
    resources,
    lng: detectInitialLanguage(),
    fallbackLng: "en",
    supportedLngs: ["en", "zh-CN", "zh-TW", "ja-JP"],
    defaultNS: "common",
    ns: I18N_NAMESPACES as unknown as string[],
    interpolation: {
      // React already escapes — i18next double-escaping would mangle
      // text with `<` or `&` (e.g. version numbers wrapped in tags).
      escapeValue: false,
    },
    returnNull: false,
  });
}

/** Apply a saved preference. `system` re-runs detection so the user can
 *  swap macOS Preferred Languages and have the app catch up next launch
 *  (or after they pick "System" in the dropdown). */
export function applyLanguage(pref: LanguagePref): void {
  const next: SupportedLanguage =
    pref === "system" ? detectInitialLanguage() : pref;
  if (i18n.language !== next) {
    void i18n.changeLanguage(next);
  }
}

function detectInitialLanguage(): SupportedLanguage {
  // navigator.languages reflects the user's macOS Preferred Languages list
  // in Tauri's WebKit. The first entry the system supports wins, so a user
  // with [zh-TW, en] gets Traditional even though we'd also support English.
  if (typeof navigator !== "undefined") {
    for (const candidate of navigator.languages ?? [navigator.language]) {
      const mapped = resolveSupportedLanguage(candidate);
      if (mapped !== "en" || /^en\b/i.test(candidate)) {
        return mapped;
      }
    }
  }
  return "en";
}

export { i18n };
