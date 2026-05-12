// Mirrors the Rust types in `linkpilot-core`. Kept narrow on purpose — the
// frontend only needs the fields it renders.

export type BrowserId = string;

export type BrowserKind =
  | "chromium"
  | "firefox"
  | "safari"
  | "arc"
  | "unknown";

export interface BrowserTarget {
  browser: BrowserId;
  profile?: string | null;
  workspace?: string | null;
  incognito?: boolean;
  new_window?: boolean;
}

export interface InstalledBrowser {
  id: BrowserId;
  display_name: string;
  kind: BrowserKind;
  executable: string;
  platform_app_id?: string | null;
  profile_root?: string | null;
}

export interface BrowserProfile {
  id: string;
  display_name: string;
  avatar_url?: string | null;
  email?: string | null;
}

export type MatcherTree =
  | { op: "always" }
  | { op: "all"; of: MatcherTree[] }
  | { op: "any"; of: MatcherTree[] }
  | { op: "not"; of: MatcherTree }
  | { op: "url-host"; pattern: string }
  | { op: "url-path"; pattern: string }
  | { op: "source-app"; name: string }
  | { op: "source-browser"; browser: string }
  | { op: "source-profile"; profile: string };

export type Action =
  | { kind: "open"; target: BrowserTarget }
  | { kind: "keep-source" }
  | { kind: "ask" }
  | { kind: "block" };

export interface Rule {
  id: string;
  priority: number;
  enabled: boolean;
  when: MatcherTree;
  then: Action;
  source: "gui" | "file" | "ts-compiled";
  note?: string | null;
}

export interface Workspace {
  id: string;
  display_name: string;
  description?: string | null;
}

export interface Settings {
  launch_at_login: boolean;
  history_retention_days?: number | null;
  record_query_strings: boolean;
}

export interface ConfigDocument {
  version: number;
  default_target: BrowserTarget;
  rules: Rule[];
  workspaces: Workspace[];
  settings: Settings;
  meta?: {
    last_writer_token?: string | null;
    last_writer?: "gui" | "file" | "cli" | "ts-compiled" | null;
  };
}

export interface RoutingContext {
  url: string;
  source: {
    type: "system" | "browser-extension" | "cli";
    app_name?: string | null;
    bundle_id?: string | null;
    browser?: string | null;
    profile?: string | null;
  };
}

export type RoutingDecision =
  | {
      action: "open";
      target: BrowserTarget;
      matched_rule?: string | null;
      reason: string;
    }
  | { action: "allow"; reason: string }
  | { action: "ask"; candidates: BrowserTarget[]; reason: string }
  | { action: "block"; reason: string };

export interface RouteRecord {
  timestamp_ms: number;
  context: RoutingContext;
  decision: RoutingDecision;
  matched_rule?: string | null;
}

export interface DoctorReport {
  daemon_version: string;
  is_default_browser: boolean;
  config_path?: string | null;
  installed_browser_count: number;
  ipc_socket_path?: string | null;
}

export type SetDefaultOutcome =
  | { kind: "done" }
  | { kind: "user-consent-required"; instructions_url?: string | null }
  | { kind: "not-supported" };
