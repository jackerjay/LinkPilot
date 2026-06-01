// Typed wrappers around the Tauri commands exposed by `apps/desktop/src-tauri/src/commands`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BrowserId,
  BrowserProfile,
  ConfigDocument,
  DoctorReport,
  Explained,
  InstalledBrowser,
  LanguagePref,
  PickerStyle,
  RouteRecord,
  RoutingDecision,
  Rule,
  SetDefaultOutcome,
  Suggestion,
  Workspace,
} from "./types";

export interface RouteRequest {
  url: string;
  from_app?: string | null;
  /** Bundle id of the source app. Captured by AppPickerButton on the
   *  Test URL page; without this, source-app rules authored via the
   *  picker (which store a bundle_id on the rule) would only match
   *  when the routing context also carries one — fine in production
   *  but breaks the simulator. With the bundle id present, matching
   *  also tolerates localized display names. */
  from_app_bundle_id?: string | null;
  from_browser?: string | null;
  from_profile?: string | null;
}

export const ipc = {
  configGet: () => invoke<ConfigDocument>("config_get"),
  configReplace: (doc: ConfigDocument) =>
    invoke<void>("config_replace", { doc }),

  ruleUpsert: (rule: Rule) => invoke<void>("rule_upsert", { rule }),
  ruleDelete: (id: string) => invoke<void>("rule_delete", { id }),

  workspaceUpsert: (workspace: Workspace) =>
    invoke<void>("workspace_upsert", { workspace }),
  workspaceDelete: (id: string) =>
    invoke<void>("workspace_delete", { id }),
  workspaceSetEnabled: (id: string, enabled: boolean) =>
    invoke<void>("workspace_set_enabled", { id, enabled }),

  listBrowsers: () => invoke<InstalledBrowser[]>("list_browsers"),
  addCustomBrowser: (browser: InstalledBrowser) =>
    invoke<void>("add_custom_browser", { browser }),
  removeCustomBrowser: (id: BrowserId) =>
    invoke<void>("remove_custom_browser", { id }),
  browserSetEnabled: (id: string, enabled: boolean) =>
    invoke<void>("browser_set_enabled", { id, enabled }),
  listProfiles: (browser: BrowserId) =>
    invoke<BrowserProfile[]>("list_profiles", { browser }),

  routeEvaluate: (request: RouteRequest) =>
    invoke<Explained>("route_evaluate", { request }),
  routeOpen: (request: RouteRequest) =>
    invoke<RoutingDecision>("route_open", { request }),
  routeHistory: (limit = 100) =>
    invoke<RouteRecord[]>("route_history", { limit }),

  isDefaultBrowser: () => invoke<boolean>("is_default_browser"),
  requestSetDefaultBrowser: () =>
    invoke<SetDefaultOutcome>("request_set_default_browser"),

  setPickerStyle: (style: PickerStyle) =>
    invoke<void>("set_picker_style", { style }),
  /** Persist the UI language preference. `system` means follow OS. */
  setLanguage: (language: LanguagePref) =>
    invoke<void>("set_language", { language }),
  /** Persist the "launch at login" preference and (un)install the macOS
   *  LaunchAgent that actually backs it. Plain `configReplace` only writes
   *  the JSON flag — it never touches the plist, so the toggle looked
   *  inert before this. */
  setLaunchAtLogin: (enabled: boolean) =>
    invoke<void>("set_launch_at_login", { enabled }),
  /** Persist a per-browser visible profile ordering. Empty `profileIds`
   *  clears customization for that browser — picker falls back to default
   *  sort and shows every detected profile. */
  setProfileOrder: (browser: string, profileIds: string[]) =>
    invoke<void>("set_profile_order", { browser, profileIds }),
  /** Open the picker window with real browsers/profile ordering. The picked
   *  target opens `testUrl`, so Settings can verify focus and profile routing. */
  pickerPreview: (testUrl: string) =>
    invoke<void>("picker_preview", { testUrl }),

  doctor: () => invoke<DoctorReport>("doctor"),
  importConfig: (path: string) => invoke<void>("import_config", { path }),
  exportConfig: (path: string) => invoke<void>("export_config", { path }),
  updateFetchMetadata: (request: UpdateMetadataRequest) =>
    invoke<UpdateCheckResult>("update_fetch_metadata", { request }),
  updateDownload: (request: UpdateDownloadRequest) =>
    invoke<UpdateDownload>("update_download", { request }),

  appIcon: (request: AppIconRequest) =>
    invoke<AppIcon | null>("app_icon", { request }),
  pickApp: () => invoke<PickedApp | null>("pick_app"),

  cliInstallStatus: () => invoke<CliInstallStatus>("cli_install_status"),
  cliInstallToPath: (target?: string) =>
    invoke<string>("cli_install_to_path", { target: target ?? null }),

  daemonServiceStatus: () => invoke<DaemonServiceStatus>("daemon_service_status"),
  daemonServiceInstall: () => invoke<DaemonServiceStatus>("daemon_service_install"),
  daemonServiceUninstall: () => invoke<DaemonServiceStatus>("daemon_service_uninstall"),

  suggestionsList: () => invoke<Suggestion[]>("suggestions_list"),
  suggestionsDismiss: (
    host: string,
    browser_id: string,
    profile_id?: string | null,
  ) =>
    invoke<void>("suggestions_dismiss", {
      host,
      browserId: browser_id,
      profileId: profile_id ?? null,
    }),
  observationsClear: () => invoke<void>("observations_clear"),
  observationsExport: (dest: string) =>
    invoke<void>("observations_export", { dest }),
};

export interface CliInstallStatus {
  /** Absolute path of `lpt` inside the running .app bundle, or `null` for
   *  dev builds where the embed step hasn't run. */
  bundled_path: string | null;
  /** `~/.local/bin/lpt` — what `cliInstallToPath()` writes by default. */
  default_target: string;
  /** True iff `default_target` already points at `bundled_path`. */
  already_installed: boolean;
}

export interface DaemonServiceStatus {
  /** Path of the bundled `linkpilot-daemon` binary inside the running
   *  .app, or null on dev builds. */
  bundled_path: string | null;
  /** Whether `~/Library/LaunchAgents/app.linkpilot.daemon.plist` exists. */
  plist_exists: boolean;
  /** Whether `launchctl list app.linkpilot.daemon` finds the agent. */
  loaded: boolean;
  /** PID of the running daemon, if launchd has it active. */
  pid: number | null;
  /** Whether the GUI hosts the daemon itself ("in-process") or is
   *  talking to a separately-running `linkpilot-daemon` ("external"). */
  gui_mode: "in-process" | "external";
}

export interface UpdateMetadataRequest {
  currentVersion: string;
}

export interface UpdateAssetMeta {
  name: string;
  downloadUrl: string;
  size: number | null;
  /** Lowercase hex SHA-256 from the release's `checksums.txt`. `null`
   *  when the file is missing or doesn't list this asset — the
   *  renderer then surfaces "checksumMissing" and refuses to call
   *  `updateDownload`. */
  sha256: string | null;
}

export interface UpdateCheckResult {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  asset: UpdateAssetMeta;
  releaseName: string | null;
  publishedAt: string | null;
  available: boolean;
  checkedAt: number;
}

export interface UpdateDownloadRequest {
  url: string;
  version: string;
  asset_name: string;
  expected_bytes?: number | null;
  /** Lowercase hex SHA-256 from `checksums.txt`. Forwarded to the
   *  daemon so it can verify the downloaded DMG before moving it into
   *  place. `null` makes the daemon refuse to write — we never auto-
   *  install an unverified binary. */
  expected_sha256?: string | null;
}

export interface UpdateDownload {
  version: string;
  asset_name: string;
  path: string;
  already_downloaded: boolean;
  bytes: number;
}

export interface AppIconRequest {
  bundle_id?: string | null;
  app_path?: string | null;
  /** Display name (Spotlight fallback) when neither bundle_id nor
   *  app_path is known — e.g. for a source-app matcher that only
   *  stored the human name "Slack". */
  name?: string | null;
  /** Pixel size of the longest edge. Defaults to 64 in Rust. */
  size?: number;
}

export interface AppIcon {
  /** `data:image/png;base64,…` — drop directly into `<img src>`. */
  data_url: string;
}

export interface PickedApp {
  name: string;
  bundle_id: string;
  /** POSIX path to the .app bundle. May be empty if osascript couldn't
   *  resolve the path; callers should fall back to using `name` for
   *  `open -a` style launching. */
  app_path: string;
}

export type RouteLoggedHandler = (record: RouteRecord) => void;
export const onRouteLogged = async (
  handler: RouteLoggedHandler,
): Promise<UnlistenFn> => listen<RouteRecord>("route-logged", (e) => handler(e.payload));

export const onConfigChanged = async (
  handler: () => void,
): Promise<UnlistenFn> => listen("config-changed", () => handler());
