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
  /** Deterministic hex (`#RRGGBB`) accent color for this profile.
   *  The picker uses it for the Halo wheel; Inspector reuses it for
   *  the matched-target line. `null` for browsers we can't introspect
   *  (Safari, Unknown). */
  accent_color?: string | null;
  /** True for Chromium's `Default` profile, Firefox's `Default=1`. */
  is_default?: boolean;
}

export type MatcherTree =
  | { op: "always" }
  | { op: "all"; of: MatcherTree[] }
  | { op: "any"; of: MatcherTree[] }
  | { op: "not"; of: MatcherTree }
  | { op: "url-host"; pattern: string }
  | { op: "url-path"; pattern: string }
  | { op: "source-app"; name: string; bundle_id?: string | null }
  | { op: "source-browser"; browser: string }
  | { op: "source-profile"; profile: string };

export type Action =
  | { kind: "open"; target: BrowserTarget }
  | { kind: "keep-source" }
  | { kind: "ask" }
  | { kind: "block" };

// Per-node match trace for the matched rule. Mirrors `routing::MatcherEval`
// on the Rust side. Same shape as `MatcherTree` but every node carries a
// `matched: boolean` flag — used by the Inspector to highlight which
// sub-matcher made the rule fire.
export type MatcherEval =
  | { op: "always"; matched: boolean }
  | { op: "all"; matched: boolean; of: MatcherEval[] }
  | { op: "any"; matched: boolean; of: MatcherEval[] }
  | { op: "not"; matched: boolean; of: MatcherEval }
  | { op: "url-host"; matched: boolean; pattern: string }
  | { op: "url-path"; matched: boolean; pattern: string }
  | { op: "source-app"; matched: boolean; name: string }
  | { op: "source-browser"; matched: boolean; browser: string }
  | { op: "source-profile"; matched: boolean; profile: string };

export interface Rule {
  id: string;
  enabled: boolean;
  when: MatcherTree;
  then: Action;
  source: "gui" | "file" | "ts-compiled";
  note?: string | null;
  /** Optional workspace this rule belongs to. When the referenced
   *  workspace is disabled, the router skips this rule even if
   *  `enabled` is true. `null`/undefined = ungrouped. */
  workspace_id?: string | null;
}

export interface Workspace {
  id: string;
  display_name: string;
  description?: string | null;
  /** Batch on/off for every rule that targets this workspace. */
  enabled: boolean;
}

export interface Settings {
  launch_at_login: boolean;
  history_retention_days?: number | null;
  record_query_strings: boolean;
  /** When true, the desktop app checks GitHub Releases once on startup
   *  and surfaces newer builds in Settings. */
  auto_check_updates: boolean;
  /** Master kill-switch for rule evaluation. When false the router
   *  bypasses all rules and opens every link in `default_target`. The
   *  tray popover's "Smart routing" toggle flips this. */
  smart_routing_enabled: boolean;
  /** Visual style for the browser+profile picker. Set via the
   *  Settings page or `lpt settings picker-style …`; the picker
   *  window reads it once on open. */
  picker_style: PickerStyle;
  /** Per-browser visible profile ordering. Keys are browser ids ("chrome",
   *  "edge", "arc"). Empty/missing means default sort. A non-empty list is
   *  the complete visible Halo inventory; profiles missing from the list are
   *  hidden until added back in Settings. */
  profile_orders: Record<string, string[]>;
}

/** Mirrors `core::config::PickerStyle`. */
export type PickerStyle = "frosted" | "bezel" | "crown";

export interface ConfigDocument {
  version: number;
  default_target: BrowserTarget;
  rules: Rule[];
  workspaces: Workspace[];
  /** User-added browsers that bypass auto-detection. Merged into
   *  `list_browsers` server-side; the Browsers page surfaces these as
   *  removable entries with a "custom" tag. */
  custom_browsers: InstalledBrowser[];
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

// Mirrors `routing::Explained`: a decision + the matched rule's eval tree.
// Returned by the Tauri `route_evaluate` command — drives the Test-URL panel
// (and matches what's also recorded in RouteRecord.explanation).
export interface Explained {
  decision: RoutingDecision;
  explanation: MatcherEval | null;
}

export interface RouteRecord {
  timestamp_ms: number;
  context: RoutingContext;
  decision: RoutingDecision;
  matched_rule?: string | null;
  // `null` when no user rule fired (default-target fallback) or when the
  // record predates the explanation feature.
  explanation?: MatcherEval | null;
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
