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
  RouteRecord,
  RoutingDecision,
  Rule,
  SetDefaultOutcome,
  Workspace,
} from "./types";

export interface RouteRequest {
  url: string;
  from_app?: string | null;
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

  doctor: () => invoke<DoctorReport>("doctor"),
  importConfig: (path: string) => invoke<void>("import_config", { path }),
  exportConfig: (path: string) => invoke<void>("export_config", { path }),

  appIcon: (request: AppIconRequest) =>
    invoke<AppIcon | null>("app_icon", { request }),
  pickApp: () => invoke<PickedApp | null>("pick_app"),
};

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
