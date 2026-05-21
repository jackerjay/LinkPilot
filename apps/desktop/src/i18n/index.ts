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
import en_browsers from "./locales/en/browsers.json";
import en_common from "./locales/en/common.json";
import en_inspector from "./locales/en/inspector.json";
import en_menuBar from "./locales/en/menuBar.json";
import en_onboarding from "./locales/en/onboarding.json";
import en_picker from "./locales/en/picker.json";
import en_rules from "./locales/en/rules.json";
import en_settings from "./locales/en/settings.json";
import en_testUrl from "./locales/en/testUrl.json";
import en_tray from "./locales/en/tray.json";
import en_workspace from "./locales/en/workspace.json";

import zhCn_app from "./locales/zh-CN/app.json";
import zhCn_browsers from "./locales/zh-CN/browsers.json";
import zhCn_common from "./locales/zh-CN/common.json";
import zhCn_inspector from "./locales/zh-CN/inspector.json";
import zhCn_menuBar from "./locales/zh-CN/menuBar.json";
import zhCn_onboarding from "./locales/zh-CN/onboarding.json";
import zhCn_picker from "./locales/zh-CN/picker.json";
import zhCn_rules from "./locales/zh-CN/rules.json";
import zhCn_settings from "./locales/zh-CN/settings.json";
import zhCn_testUrl from "./locales/zh-CN/testUrl.json";
import zhCn_tray from "./locales/zh-CN/tray.json";
import zhCn_workspace from "./locales/zh-CN/workspace.json";

import zhTw_app from "./locales/zh-TW/app.json";
import zhTw_browsers from "./locales/zh-TW/browsers.json";
import zhTw_common from "./locales/zh-TW/common.json";
import zhTw_inspector from "./locales/zh-TW/inspector.json";
import zhTw_menuBar from "./locales/zh-TW/menuBar.json";
import zhTw_onboarding from "./locales/zh-TW/onboarding.json";
import zhTw_picker from "./locales/zh-TW/picker.json";
import zhTw_rules from "./locales/zh-TW/rules.json";
import zhTw_settings from "./locales/zh-TW/settings.json";
import zhTw_testUrl from "./locales/zh-TW/testUrl.json";
import zhTw_tray from "./locales/zh-TW/tray.json";
import zhTw_workspace from "./locales/zh-TW/workspace.json";

import jaJp_app from "./locales/ja-JP/app.json";
import jaJp_browsers from "./locales/ja-JP/browsers.json";
import jaJp_common from "./locales/ja-JP/common.json";
import jaJp_inspector from "./locales/ja-JP/inspector.json";
import jaJp_menuBar from "./locales/ja-JP/menuBar.json";
import jaJp_onboarding from "./locales/ja-JP/onboarding.json";
import jaJp_picker from "./locales/ja-JP/picker.json";
import jaJp_rules from "./locales/ja-JP/rules.json";
import jaJp_settings from "./locales/ja-JP/settings.json";
import jaJp_testUrl from "./locales/ja-JP/testUrl.json";
import jaJp_tray from "./locales/ja-JP/tray.json";
import jaJp_workspace from "./locales/ja-JP/workspace.json";

import {
  resolveSupportedLanguage,
  type LanguagePref,
  type SupportedLanguage,
} from "./languages";

export const I18N_NAMESPACES = [
  "common",
  "app",
  "settings",
  "rules",
  "workspace",
  "inspector",
  "browsers",
  "testUrl",
  "menuBar",
  "tray",
  "picker",
  "onboarding",
] as const;
export type I18nNamespace = (typeof I18N_NAMESPACES)[number];

const resources = {
  en: {
    common: en_common,
    app: en_app,
    settings: en_settings,
    rules: en_rules,
    workspace: en_workspace,
    inspector: en_inspector,
    browsers: en_browsers,
    testUrl: en_testUrl,
    menuBar: en_menuBar,
    tray: en_tray,
    picker: en_picker,
    onboarding: en_onboarding,
  },
  "zh-CN": {
    common: zhCn_common,
    app: zhCn_app,
    settings: zhCn_settings,
    rules: zhCn_rules,
    workspace: zhCn_workspace,
    inspector: zhCn_inspector,
    browsers: zhCn_browsers,
    testUrl: zhCn_testUrl,
    menuBar: zhCn_menuBar,
    tray: zhCn_tray,
    picker: zhCn_picker,
    onboarding: zhCn_onboarding,
  },
  "zh-TW": {
    common: zhTw_common,
    app: zhTw_app,
    settings: zhTw_settings,
    rules: zhTw_rules,
    workspace: zhTw_workspace,
    inspector: zhTw_inspector,
    browsers: zhTw_browsers,
    testUrl: zhTw_testUrl,
    menuBar: zhTw_menuBar,
    tray: zhTw_tray,
    picker: zhTw_picker,
    onboarding: zhTw_onboarding,
  },
  "ja-JP": {
    common: jaJp_common,
    app: jaJp_app,
    settings: jaJp_settings,
    rules: jaJp_rules,
    workspace: jaJp_workspace,
    inspector: jaJp_inspector,
    browsers: jaJp_browsers,
    testUrl: jaJp_testUrl,
    menuBar: jaJp_menuBar,
    tray: jaJp_tray,
    picker: jaJp_picker,
    onboarding: jaJp_onboarding,
  },
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
export function applyLanguage(pref: LanguagePref): Promise<void> {
  const next: SupportedLanguage =
    pref === "system" ? detectInitialLanguage() : pref;
  if (i18n.language !== next) {
    return i18n.changeLanguage(next).then(() => undefined);
  }
  return Promise.resolve();
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
