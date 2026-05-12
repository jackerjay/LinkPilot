// Typed wrappers around the Tauri commands exposed by `apps/desktop/src-tauri/src/commands`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BrowserId,
  BrowserProfile,
  ConfigDocument,
  DoctorReport,
  InstalledBrowser,
  RouteRecord,
  RoutingDecision,
  Rule,
  SetDefaultOutcome,
} from "./types";

export interface RouteRequest {
  url: string;
  from_app?: string | null;
}

export const ipc = {
  configGet: () => invoke<ConfigDocument>("config_get"),
  configReplace: (doc: ConfigDocument) =>
    invoke<void>("config_replace", { doc }),

  ruleUpsert: (rule: Rule) => invoke<void>("rule_upsert", { rule }),
  ruleDelete: (id: string) => invoke<void>("rule_delete", { id }),

  listBrowsers: () => invoke<InstalledBrowser[]>("list_browsers"),
  listProfiles: (browser: BrowserId) =>
    invoke<BrowserProfile[]>("list_profiles", { browser }),

  routeEvaluate: (request: RouteRequest) =>
    invoke<RoutingDecision>("route_evaluate", { request }),
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
};

export type RouteLoggedHandler = (record: RouteRecord) => void;
export const onRouteLogged = async (
  handler: RouteLoggedHandler,
): Promise<UnlistenFn> => listen<RouteRecord>("route-logged", (e) => handler(e.payload));

export const onConfigChanged = async (
  handler: () => void,
): Promise<UnlistenFn> => listen("config-changed", () => handler());
