// Public entry point for `@linkpilot/config`. End users write a
// `linkpilot.config.ts` like:
//
//   import { defineConfig, browser, route } from "@linkpilot/config";
//
//   export default defineConfig({
//     defaultTarget: browser.arc(),
//     rules: [
//       route.host("github.com").to(browser.chrome.profile("Work")),
//       route.host("figma.com").to(browser.arc()),
//     ],
//   });
//
// and then `lp config compile linkpilot.config.ts` runs the file through
// `bun`, captures the printed JSON, and feeds it to `ConfigStore::replace`.
//
// `defineConfig` returns a `DslConfig` that is *not* yet the
// `ConfigDocument` JSON. Use `compile(cfg)` to get the wire shape;
// `lp config compile` does this for you, but anyone embedding the DSL
// programmatically (tests, custom build scripts) can call it directly.

export { Target, browser } from "./targets.js";
export type { BrowserHandle } from "./targets.js";
export { RouteBuilder, PendingRule, route } from "./matchers.js";
export { compile, printConfig } from "./compile.js";
export type {
  BrowserId,
  BrowserTargetJson,
  MatcherTreeJson,
  ActionJson,
  RuleSourceJson,
  RuleJson,
  WorkspaceJson,
  SettingsJson,
  ConfigDocumentJson,
} from "./types.js";

import type { Target } from "./targets.js";
import type { PendingRule } from "./matchers.js";

/**
 * Workspace declaration. `enabled` defaults to true; set false to ship
 * a workspace that the user can flip on later from the GUI without
 * losing per-rule state.
 */
export interface DslWorkspace {
  id: string;
  displayName: string;
  description?: string;
  enabled?: boolean;
}

/**
 * Daemon-level settings. Field names match the DSL (camelCase); the
 * compiler maps them to snake_case for the wire shape. Every field is
 * optional — omitted ones inherit the daemon default.
 */
export interface DslSettings {
  launchAtLogin?: boolean;
  historyRetentionDays?: number | null;
  recordQueryStrings?: boolean;
  smartRoutingEnabled?: boolean;
}

export interface DslConfig {
  /** Browser to fall through to when no rule matches. */
  defaultTarget: Target;
  rules: PendingRule[];
  workspaces?: DslWorkspace[];
  settings?: DslSettings;
}

/**
 * Pass through your config so the DSL type system can lint it before
 * `bun` runs the file. Functionally a no-op; mirrors what zod / vite /
 * tanstack-router do for editor surface.
 */
export function defineConfig(cfg: DslConfig): DslConfig {
  return cfg;
}
