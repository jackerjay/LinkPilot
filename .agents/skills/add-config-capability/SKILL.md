---
name: add-config-capability
description: |
  Use this skill when extending LinkPilot's user-configurable surface —
  adding a new rule matcher, action variant, Settings field, Workspace
  property, browser metadata field, or any new key that lands in the
  on-disk `ConfigDocument`. The project has a layered architecture
  (core schema → daemon Tauri command → typed frontend wrapper → React
  page → `lpt` CLI subcommand) and a half-wired capability (e.g. a new
  rule matcher with no CLI, or a setting the GUI doesn't surface) is
  the typical drift. This skill walks through every layer so nothing
  ships half-done.
---

# Adding a new config capability to LinkPilot

LinkPilot persists every user-facing configuration in a single JSON
document at `~/Library/Application Support/LinkPilot/linkpilot.config.json`.
That document is owned by `linkpilot-core::config::ConfigDocument` and
threaded through the daemon (Tauri shell), the GUI, and the `lpt` CLI.
A new capability is usually "land a field in `ConfigDocument` and make
it editable everywhere a user could expect to edit it."

Skipping any layer below leaves the capability half-wired. If the user
is on a hard deadline they may explicitly choose to defer one or two —
that's fine, but it should be a conscious call, not an oversight.

## Layer checklist

### 1. Core schema — `crates/core/src/`

Decide which file the new field belongs in:

- New **rule matcher** (e.g. matching on time-of-day, VPN state) →
  add a variant to `MatcherTree` in `rules.rs`. Add a router branch in
  `routing.rs::evaluate_matcher` (or wherever `MatcherTree` is matched
  exhaustively) and add a unit test in the same file's `mod tests`.
- New **action** (e.g. "open in incognito if not already") → add a
  variant to `Action` in `rules.rs`. Handle it in `dispatch.rs` in
  `apps/desktop/src-tauri/src/`.
- New **settings field** (a global toggle) → add to `Settings` in
  `config/mod.rs`. Default value goes in `Settings::default()`.
  If the field is non-`Option` and old configs predate it, mark with
  `#[serde(default = "…")]` or `#[serde(default)]` so old JSON loads.
- New **workspace property** → add to `Workspace` in `config/mod.rs`.
- New **browser metadata field** → add to `InstalledBrowser` or
  `BrowserProfile` in `browser.rs`. Both auto-detect (platform crate)
  and custom-add paths need to populate it.

Run `cargo test -p linkpilot-core` to confirm round-trip serde + router
behavior. The router tests use small inline fixtures — copy one when
adding a matcher.

### 2. Daemon — `apps/desktop/src-tauri/src/`

The daemon owns the live `ConfigStore` and exposes mutators as Tauri
commands. Most capabilities don't need a NEW command — `config_replace`
already accepts the whole document, and `rule_upsert` / `workspace_upsert`
already cover those entities. Add a dedicated command only when:

- The mutation needs a side effect beyond writing the config (e.g.
  `set_smart_routing` flips a routing kill-switch that the GUI's tray
  popover toggles — it's a one-key write but the tray needs an atomic
  command).
- The capability is platform-backed (e.g. `request_set_default_browser`
  calls into `linkpilot-platform-mac`).

Files to touch:

- `commands/mod.rs` — add the `#[tauri::command]` function.
- `lib.rs` — register it in the `tauri::generate_handler![...]` macro.
- `capabilities/default.json` — list the command in `permissions` so
  the renderer is actually allowed to call it.

### 3. Frontend wrapper — `apps/desktop/src/lib/tauri.ts`

Add a typed wrapper. The pattern:

```ts
export async function myCommand(args: { foo: string }): Promise<MyResult> {
  return invoke<MyResult>("my_command", args);
}
```

Keep type definitions co-located if they're command-specific; promote
to `lib/types.ts` if they're shared. Generated types are NOT used —
this is hand-maintained.

### 4. Frontend page — `apps/desktop/src/pages/`

Each capability surfaces in exactly one page:

- Rule matchers / actions → `RulesPage.tsx` + `RuleEditor.tsx`.
- Settings → `SettingsPage.tsx`.
- Workspaces → `WorkspacesPage.tsx`.
- Browsers (auto-detected + custom) → `BrowsersPage.tsx`.
- Default-browser registration → `SettingsPage.tsx` (top-of-page
  "Set as Default" card).

The structured rule editor in `RuleEditor.tsx` is the most involved
when adding a matcher — it has to render an input control for every
`MatcherTree` variant. The "Advanced: raw JSON" fallback gives users
an escape hatch for anything the structured editor can't express yet.

### 5. CLI — `crates/cli/src/main.rs`

The `lpt` CLI mirrors every configurable thing in the GUI (see commit
`968ffea` for the full surface). Pick the right subcommand group:

| Capability | CLI subcommand group |
|---|---|
| Rule matcher | `lpt rules add` — add a flag (e.g. `--time-window`) AND extend `build_matcher` |
| Action | `lpt rules add` — add a flag (e.g. `--always-incognito`) AND extend `build_action` |
| Settings field | `lpt settings <name> <value>` — add a new `SettingsAction` variant |
| Workspace property | `lpt workspaces add` — add a flag |
| Browser metadata | `lpt browsers custom add` — add a flag |
| Default target tweak | `lpt config set-default-target` — add a flag |

Writes go through `mutate_local` (atomic rewrite + fsnotify echo via
the existing anti-echo token), so a running daemon picks up the change
automatically — no new IPC endpoints needed.

If the matcher tree / action shape can't be expressed with flag
combinations, users can still hit it via `--when-json '{...}'` /
`--then-json '{...}'`. Document the escape hatch in the README example
block if the flag form is awkward.

### 6. Tests

- `cargo test -p linkpilot-core` — router + serde round-trip.
- `cargo build -p linkpilot-cli` then run `lpt <cmd> --help` and a
  quick `--dry-run` smoke against a `/tmp/linkpilot.json` config.
- For GUI: `cd apps/desktop && npx tauri dev` and click through the
  new control.

### 7. Docs

If the capability is user-facing (not internal plumbing), update:

- `README.md` — the "CLI mirrors everything the GUI can configure"
  block, or the relevant section.
- `README.zh.md` — mirror the same change.
- `AGENTS.md` — only if the capability changes a project-wide
  convention worth surfacing to future sessions.

## Anti-patterns

- **GUI-only field.** A setting only the GUI can flip means `lp` and
  TypeScript DSL users can't script around it. Either add it to the
  CLI in the same commit or open a follow-up issue.
- **CLI-only field.** Inverse — power users can flip it but the GUI
  silently ignores changes. The frontend will at least show it via
  `config_get` JSON dump (Config page's "Advanced: raw JSON" area),
  but that's a footgun, not a feature.
- **New `Request`/`Response` variant added for something that's just
  a config field.** Almost never needed — `ConfigGet` + `ConfigReplace`
  (or the higher-level `rule_upsert` / `workspace_upsert`) handles
  arbitrary doc mutations. Add a dedicated IPC verb only when the
  daemon does work the client can't (platform handshake, in-memory
  state).
- **Forgetting `#[serde(default)]` on a new field.** Old configs on
  disk won't deserialize. Bump `SCHEMA_VERSION` in
  `crates/core/src/config/mod.rs` only when you've made a breaking
  shape change (rare); a backfilled default is preferable.

## Quick reference — files most likely to need edits

```
crates/core/src/config/mod.rs        # Settings, Workspace, ConfigDocument schema
crates/core/src/rules.rs             # MatcherTree, Action variants
crates/core/src/browser.rs           # InstalledBrowser, BrowserProfile, BrowserTarget
crates/core/src/routing.rs           # Router::evaluate, matcher branches + tests
apps/desktop/src-tauri/src/commands/mod.rs   # Tauri command handlers
apps/desktop/src-tauri/src/lib.rs            # generate_handler! registration
apps/desktop/src-tauri/capabilities/default.json
apps/desktop/src/lib/tauri.ts        # typed wrappers
apps/desktop/src/pages/*.tsx         # the page that surfaces it
crates/cli/src/main.rs               # lp subcommand + arg parsing
```
