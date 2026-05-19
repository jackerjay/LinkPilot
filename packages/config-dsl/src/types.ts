// Wire types mirroring the Rust ConfigDocument schema in
// crates/core/src/{config/mod.rs, rules.rs, browser.rs}. Field names use
// snake_case so JSON.stringify yields exactly what the daemon expects —
// no key-renaming layer on either side.
//
// We don't depend on a runtime JSON schema library; the compile() output
// is structural-typed against these interfaces. Misshape would fail at
// the daemon's serde deserialise, which is acceptable for a DSL whose
// users are expected to run `lp config compile` and see the error
// reported by Rust.

/** A browser id like "chrome", "arc", or a custom one the user defined. */
export type BrowserId = string;

/** Routing target wire-shape — must match crates/core BrowserTarget. */
export interface BrowserTargetJson {
  browser: BrowserId;
  profile: string | null;
  workspace: string | null;
  incognito: boolean;
  new_window: boolean;
}

/** Discriminated union for the matcher AST. `op` is the tag, kebab-case. */
export type MatcherTreeJson =
  | { op: "always" }
  | { op: "all"; of: MatcherTreeJson[] }
  | { op: "any"; of: MatcherTreeJson[] }
  | { op: "not"; of: MatcherTreeJson }
  | { op: "url-host"; pattern: string }
  | { op: "url-path"; pattern: string }
  | { op: "source-app"; name: string; bundle_id: string | null }
  | { op: "source-browser"; browser: string }
  | { op: "source-profile"; profile: string };

/** Action wire-shape. `kind` is the tag, kebab-case. */
export type ActionJson =
  | { kind: "open"; target: BrowserTargetJson }
  | { kind: "keep-source" }
  | { kind: "ask" }
  | { kind: "block" };

export type RuleSourceJson = "gui" | "file" | "ts-compiled";

/** A single rule as the daemon reads it. List order in the
 *  surrounding `rules: RuleJson[]` IS priority — top wins. There is
 *  no numeric priority field; ties are structurally impossible. */
export interface RuleJson {
  id: string;
  enabled: boolean;
  when: MatcherTreeJson;
  then: ActionJson;
  source: RuleSourceJson;
  note: string | null;
  workspace_id: string | null;
}

export interface WorkspaceJson {
  id: string;
  display_name: string;
  description: string | null;
  enabled: boolean;
}

export interface SettingsJson {
  launch_at_login: boolean;
  history_retention_days: number | null;
  record_query_strings: boolean;
  smart_routing_enabled: boolean;
}

/** Top-level config document. v0.2 schema_version is 1. */
export interface ConfigDocumentJson {
  version: number;
  default_target: BrowserTargetJson;
  rules: RuleJson[];
  workspaces: WorkspaceJson[];
  custom_browsers: unknown[]; // GUI-managed; DSL never produces these.
  settings: SettingsJson;
  meta: Record<string, unknown>;
}
