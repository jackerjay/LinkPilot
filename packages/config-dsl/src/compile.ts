// DSL → wire-shape compiler.
//
// `compile()` takes the `DslConfig` your `linkpilot.config.ts` exports
// (the value returned by `defineConfig`) and produces a `ConfigDocumentJson`
// the daemon can deserialise. The two responsibilities here are:
//
//   1. Field renaming: DSL is camelCase (matches TS conventions); wire
//      shape is snake_case (matches Rust serde defaults).
//
//   2. UUID assignment + source stamping: every rule gets a fresh v4
//      UUID and `source: "ts-compiled"`. Stable ids would be nicer for
//      diffs but require user input we don't have at the DSL level; the
//      GUI's "Copy as GUI-editable" path (M4.4) re-issues an id anyway.
//
// `printConfig()` is the canonical "compile and print as JSON" helper
// that `lp config compile` reads on stdout — kept here (rather than
// expecting users to write the boilerplate) so a `linkpilot.config.ts`
// only needs `export default defineConfig({...})`.

import type { DslConfig, DslWorkspace, DslSettings } from "./index.js";
import type {
  ConfigDocumentJson,
  RuleJson,
  WorkspaceJson,
  SettingsJson,
} from "./types.js";

const SCHEMA_VERSION = 1;

/** UUID v4 generator — Node 19+/Bun ship `crypto.randomUUID()` natively. */
function uuid(): string {
  // `crypto` is globally available in Bun and Node 19+. Cast through
  // `unknown` so TypeScript doesn't insist on a DOM lib at build time.
  const c = (globalThis as unknown as { crypto?: { randomUUID?: () => string } }).crypto;
  if (c && typeof c.randomUUID === "function") {
    return c.randomUUID();
  }
  // Last-resort fallback. The daemon validates UUIDs, so this branch
  // is structurally correct even if it's not v4-random — but every
  // shipping runtime we target has crypto.randomUUID.
  return "00000000-0000-4000-8000-" + Math.random().toString(16).slice(2, 14).padEnd(12, "0");
}

function compileWorkspace(w: DslWorkspace): WorkspaceJson {
  return {
    id: w.id,
    display_name: w.displayName,
    description: w.description ?? null,
    enabled: w.enabled ?? true,
  };
}

function compileSettings(s: DslSettings | undefined): SettingsJson {
  return {
    launch_at_login: s?.launchAtLogin ?? false,
    history_retention_days: s?.historyRetentionDays ?? null,
    record_query_strings: s?.recordQueryStrings ?? false,
    smart_routing_enabled: s?.smartRoutingEnabled ?? true,
  };
}

export function compile(cfg: DslConfig): ConfigDocumentJson {
  // Rule array order IS priority — top of `cfg.rules` wins. The DSL
  // user controls this by ordering the `route.*` calls inside their
  // `defineConfig({ rules: [...] })` block.
  const rules: RuleJson[] = cfg.rules.map((r) => ({
    id: uuid(),
    enabled: r.enabled,
    when: r.when,
    then: r.then,
    source: "ts-compiled",
    note: r.note,
    workspace_id: r.workspaceId,
  }));

  return {
    version: SCHEMA_VERSION,
    default_target: cfg.defaultTarget.toJSON(),
    rules,
    workspaces: (cfg.workspaces ?? []).map(compileWorkspace),
    custom_browsers: [],
    settings: compileSettings(cfg.settings),
    meta: {},
  };
}

/**
 * Compile and emit JSON to stdout. The `lp config compile` driver reads
 * this stdout, validates it as a `ConfigDocument`, and feeds it through
 * `ConfigStore::replace`. Users who want to embed `compile()` directly
 * (custom build pipelines) can ignore this helper.
 */
export function printConfig(cfg: DslConfig): void {
  // `process.stdout.write` to avoid any chance of trailing-newline /
  // buffered-output drift between Bun and Node. The Rust side will
  // tolerate a trailing newline but the contract is "pure JSON on
  // stdout, nothing else".
  const json = JSON.stringify(compile(cfg));
  const proc = (globalThis as unknown as { process?: { stdout: { write: (s: string) => void } } })
    .process;
  if (proc?.stdout) {
    proc.stdout.write(json);
    return;
  }
  // Should never happen on Bun/Node, but keep the DSL importable in
  // generic JS sandboxes for unit tests. `console` isn't in the ES2022
  // lib we target, so reach for it through globalThis.
  const c = (globalThis as unknown as { console?: { log: (s: string) => void } }).console;
  c?.log(json);
}
