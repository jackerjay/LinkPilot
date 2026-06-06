<!-- refreshed: 2026-06-05 -->
# Architecture

**Analysis Date:** 2026-06-05

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                      Entry Points                                        │
│                                                                          │
│  macOS URL event    Tauri GUI commands    lpt CLI       NMH (v0.3)       │
│  (deep-link plugin) (invoke)             (IPC / local)  (stdio bridge)   │
└──────────┬──────────────────┬────────────────┬──────────────┬───────────┘
           │                  │                │              │
           ▼                  ▼                ▼              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Tauri Shell / GUI Daemon                            │
│  `apps/desktop/src-tauri/src/`                                           │
│                                                                          │
│  url_handler.rs     commands/mod.rs     ipc_host.rs   nmh_supervisor.rs │
│  (deep-link CB)     (Tauri invoke)      (IPC server)  (NMH supervisor)  │
│                        ↓                                                 │
│              dispatch.rs (Open/Ask/Allow/Block routing)                  │
│              picker.rs   (Cmd-Tab-style Halo picker window)              │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Core Library — `crates/core/src/`                   │
│                                                                          │
│  daemon.rs (DaemonRuntime + RequestHandler)                              │
│  routing.rs (Router — stateless pure fn over ConfigDocument)            │
│  rules.rs   (MatcherTree, Action — composable AST)                      │
│  config/    (ConfigDocument schema + ConfigStore + fsnotify watcher)    │
│  history.rs (RouteHistory — in-memory ring buffer, 1000 records)        │
│  protocol.rs (IPC Request / Response / Event types — wire format def)   │
│  platform.rs (PlatformProvider trait + StubProvider)                    │
│  browser.rs  (BrowserTarget, InstalledBrowser, BrowserProfile)          │
└──────────┬──────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Platform Backend        IPC Transport          CLI Client               │
│  `crates/platform-mac/`  `crates/ipc/`          `crates/cli/`           │
│  MacProvider             Unix socket             lpt binary              │
│  (LaunchServices,        length-prefix JSON      (reads via IPC,         │
│   NSWorkspace, objc2)    server + client         writes local config)    │
└──────────┬──────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  On-disk State                                                           │
│  ~/Library/Application Support/LinkPilot/linkpilot.config.json          │
│  ~/Library/Application Support/LinkPilot/linkpilot.sock                 │
│  ~/Library/Application Support/LinkPilot/observations.ndjson            │
│  ~/Library/Application Support/LinkPilot/linkpilot-daemon.pid           │
└─────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File(s) |
|-----------|----------------|---------|
| Router | Pure stateless evaluation of URL + source against rules | `crates/core/src/routing.rs` |
| ConfigDocument | Single schema for all on-disk user configuration | `crates/core/src/config/mod.rs` |
| ConfigStore | Thread-safe load/save/watch with anti-echo fsnotify | `crates/core/src/config/store.rs` |
| MatcherTree / Action | Composable AST for rule conditions and outcomes | `crates/core/src/rules.rs` |
| DaemonRuntime | Shared daemon state + default RequestHandler impl | `crates/core/src/daemon.rs` |
| RouteHistory | Fixed-capacity (1000) in-memory ring buffer of decisions | `crates/core/src/history.rs` |
| PlatformProvider | Trait abstraction over OS capabilities | `crates/core/src/platform.rs` |
| MacProvider | Real macOS implementation (LaunchServices, NSWorkspace) | `crates/platform-mac/src/lib.rs` |
| IPC server/client | Tokio async Unix socket, length-prefixed JSON frames | `crates/ipc/src/server.rs`, `crates/ipc/src/client.rs` |
| lpt CLI | Command-line client; reads via IPC or local, writes local | `crates/cli/src/main.rs` |
| url_handler | deep-link plugin callback → Router → dispatch | `apps/desktop/src-tauri/src/url_handler.rs` |
| dispatch | Decision → launch, with Ask-picker thread spawning | `apps/desktop/src-tauri/src/dispatch.rs` |
| DaemonHandler | Tauri IPC host; overrides RouteOpen, delegates rest | `apps/desktop/src-tauri/src/ipc_host.rs` |
| Tauri commands | Rust ↔ JS boundary; all frontend mutations go here | `apps/desktop/src-tauri/src/commands/mod.rs` |
| ipc.ts | Typed TypeScript wrappers over every Tauri command | `apps/desktop/src/lib/ipc.ts` |
| React pages | UI surfaces: rules, inspector, browsers, settings, etc. | `apps/desktop/src/pages/` |
| Picker (Halo) | Cmd-Tab-style browser wheel for Ask decisions | `apps/desktop/src/picker/` + `src-tauri/src/picker.rs` |

## Pattern Overview

**Overall:** Layered Hexagonal Architecture — platform-agnostic core wrapped by OS adapters and two client surfaces (GUI Tauri shell + headless CLI).

**Key Characteristics:**
- The routing engine (`Router`) is a pure function with no IO: given a `&ConfigDocument` and a `RoutingContext` it returns a `RoutingDecision`. All side effects (file writes, URL launches, window creation) live in outer layers.
- `ConfigDocument` is the single source of truth. All mutations are atomic rewrites (`tmp + rename`). The anti-echo token in `Config::Meta` prevents the fsnotify watcher from re-broadcasting the daemon's own writes.
- `PlatformProvider` is the only seam between core and the OS. Adding a cross-platform implementation never requires touching `core/`. Win/Linux stubs compile everywhere but return `NotSupported`.
- Daemon coexistence: the Tauri shell probes for a running `linkpilot-daemon` on startup. If found, GUI runs in "client mode" (skips IPC server bind but keeps its own ConfigStore). If not, GUI hosts the daemon in-process via `DaemonHandler`.

## Layers

**Core (pure logic):**
- Purpose: Routing engine, data model, platform trait definitions, IPC protocol types
- Location: `crates/core/src/`
- Contains: `Router`, `ConfigDocument`, `ConfigStore`, `MatcherTree`, `Action`, `DaemonRuntime`, `RouteHistory`, `PlatformProvider` trait, `protocol::{Request, Response, Event}`
- Depends on: `serde`, `uuid`, `url`, `notify`, `tokio` (for watcher thread)
- Used by: Every other crate

**Platform (OS adapters):**
- Purpose: Concrete implementations of `PlatformProvider` traits
- Location: `crates/platform-mac/src/` (real), `crates/platform-win/src/`, `crates/platform-linux/src/` (stubs)
- Contains: `MacProvider`, `MacUrlLauncher`, `MacInventory`, `MacAutostart`, `MacDefaultBrowser`, `MacOpenerDetector`
- Depends on: `linkpilot-core`, `objc2`, `core-foundation`, macOS system frameworks
- Used by: `linkpilot-desktop` (Tauri shell), `linkpilot-headless-daemon`

**IPC transport:**
- Purpose: Length-prefixed JSON framing over Unix socket; client + server halves
- Location: `crates/ipc/src/`
- Contains: `server::serve()`, `client::send()`, `transport::{read_raw_frame, write_frame}`
- Depends on: `linkpilot-core` (for protocol types), `tokio`
- Used by: `linkpilot-desktop`, `linkpilot-cli`, `linkpilot-native-host` (future)

**Tauri shell (in-process GUI daemon):**
- Purpose: Deep-link handler, IPC server host, Tauri command layer, picker UI
- Location: `apps/desktop/src-tauri/src/`
- Contains: `url_handler`, `dispatch`, `ipc_host::DaemonHandler`, `commands/*`, `picker`, `tray`, `state::AppState`
- Depends on: `linkpilot-core`, `linkpilot-ipc`, `linkpilot-platform-mac`, Tauri v2
- Used by: Nothing (binary entry point)

**React frontend:**
- Purpose: Settings UI, rules editor, inspector, browser manager, picker wheel
- Location: `apps/desktop/src/`
- Contains: Pages (`pages/`), shared components (`components/`), typed IPC wrappers (`lib/ipc.ts`), Halo picker variants (`picker/halo/`)
- Depends on: Tauri JS API (`@tauri-apps/api`), Tailwind v4, Radix UI
- Used by: Nothing (WebView entry point)

**CLI client:**
- Purpose: Terminal access to every capability the GUI exposes
- Location: `crates/cli/src/main.rs`
- Contains: `lpt` clap command tree — `open`, `doctor`, `rules`, `workspaces`, `config`, `settings`, `browsers`, `default-browser`, `daemon`
- Depends on: `linkpilot-core`, `linkpilot-ipc`, `linkpilot-platform-mac`
- Used by: Nothing (binary entry point)

## Data Flow

### Primary URL Routing Path (System URL Event → Browser Launch)

1. macOS fires `open https://...` Apple Event; `tauri-plugin-deep-link` surfaces it as a Rust callback (`apps/desktop/src-tauri/src/lib.rs:170`)
2. `url_handler::dispatch_system_url` is called (`apps/desktop/src-tauri/src/url_handler.rs:15`)
3. `platform.opener_detector().detect(...)` asks macOS for the most-recently-active app → populates `Source.app_name` + `bundle_id`
4. A `RoutingContext { url, source }` is constructed
5. `Router::new(&doc).evaluate_explained(&context)` is called — pure, no IO (`crates/core/src/routing.rs:103`)
   - If `smart_routing_enabled = false` → returns default target decision immediately
   - Otherwise walks `config.rules` in list order (first match wins)
   - Each rule's `MatcherTree` is evaluated recursively via `eval_tree()`
   - Returns `Explained { decision, explanation }` where `explanation` is a per-node boolean trace
6. `RouteRecord::with_explanation(context, decision, explanation)` is created (`crates/core/src/history.rs`)
7. `state.history.log(record)` appends to the in-memory ring buffer
8. `app.emit("route-logged", &record)` broadcasts to the frontend Inspector page
9. `dispatch::execute(app, state, &decision, &url)` carries out the decision (`apps/desktop/src-tauri/src/dispatch.rs:41`):
   - `RoutingDecision::Open { target }` → `plan_open_target()` checks if the browser is installed; falls back to default_target or Ask if missing
   - `RoutingDecision::Ask` → `spawn_ask()` on a worker thread to avoid deadlock; opens Halo picker window, blocks on user pick, then launches
   - `RoutingDecision::Allow` / `Block` → `LaunchOutcome::Skipped`

### Config Write Flow (React UI → Disk → Daemon Reload)

1. React calls `ipc.ruleUpsert(rule)` → `invoke("rule_upsert", { rule })` (`apps/desktop/src/lib/ipc.ts:41`)
2. Tauri routes to `commands::rule_upsert` (`apps/desktop/src-tauri/src/commands/mod.rs:40`)
3. Command clones `ConfigDocument`, upserts the rule, calls `config_store.replace(doc, WriterId::Gui)`
4. `ConfigStore::persist()` stamps a fresh `meta.last_writer_token` UUID and atomically writes `linkpilot.config.json.tmp` → renamed to `linkpilot.config.json` (`crates/core/src/config/store.rs:167`)
5. fsnotify fires on the directory watcher; `handle_disk_change()` re-reads the file
6. Anti-echo: `disk.last_writer_token == remembered_token` → `ChangeOrigin::Echo` → `on_change` fires but the GUI ignores its own write
7. `app.emit("config-changed", label)` still fires for any subscriber that needs to refresh (e.g. WorkspacePage, tray menu)

### CLI Write Flow (lpt → Disk → Daemon Reload)

1. `lpt rules add --host github.com --open chrome` parses via clap (`crates/cli/src/main.rs`)
2. `mutate_local()` loads `ConfigDocument` from disk, modifies it, calls `config_store.persist(WriterId::Cli)`
3. Atomic write stamps a new token; fsnotify fires in the running daemon/GUI process
4. Anti-echo check: token is new (written by CLI process, not by daemon) → `ChangeOrigin::External`
5. `guard.doc = parsed` updates the daemon's in-memory config
6. `app.emit("config-changed", "external")` triggers frontend to `ipc.configGet()` and re-render

### IPC Request Flow (lpt / NMH → GUI Daemon)

1. Client calls `linkpilot_ipc::client::send(&endpoint, Request::RouteEvaluate { ... })` (`crates/ipc/src/client.rs`)
2. `write_frame()` sends `[u32 BE length][JSON bytes]` on the Unix socket
3. Tokio IPC server (`crates/ipc/src/server.rs`) `read_raw_frame()`, deserializes to `Request`; unknown verbs get `Response::Error { code: "unknown-verb" }`
4. `DaemonHandler::handle()` matches on `Request::RouteOpen`, calls `evaluate_and_log()` + `dispatch::execute()` (`apps/desktop/src-tauri/src/ipc_host.rs`)
5. All other verbs delegated to `DaemonRuntime::handle()` (`crates/core/src/daemon.rs:129`)
6. Response serialized + written back as `[u32 BE length][JSON bytes]`

**State Management:**
- All mutable shared state lives in `AppState` (`apps/desktop/src-tauri/src/state.rs`) wrapped in `Arc<Mutex<>>` or `Arc<RwLock<>>`
- `ConfigStore` uses `Arc<Mutex<State>>` so it can be cloned cheaply across threads
- `RouteHistory` uses a `Mutex<VecDeque<RouteRecord>>` with a 1000-record cap
- The Tauri webview has no direct mutable state — all mutations round-trip through Rust commands

## Key Abstractions

**MatcherTree:**
- Purpose: Composable boolean AST for rule conditions
- Examples: `crates/core/src/rules.rs:73`
- Pattern: Recursive enum — `Always`, `All { of }`, `Any { of }`, `Not { of }`, `UrlHost { pattern }`, `UrlPath { pattern }`, `SourceApp { name, bundle_id }`, `SourceBrowser`, `SourceProfile`
- `MatcherEval` mirrors this shape but carries a `matched: bool` at each node for the Inspector explain-why trace

**PlatformProvider:**
- Purpose: Single trait that groups all OS capabilities behind a stable interface
- Examples: `crates/core/src/platform.rs:29`, `crates/platform-mac/src/lib.rs:26`
- Pattern: Trait object (`Arc<dyn PlatformProvider>`); returns sub-trait references for each capability domain. `StubProvider` returns `NotSupported` everywhere and enables workspace-wide `cargo check` on Linux/Windows.

**DaemonRuntime:**
- Purpose: Shared daemon state (ConfigStore + RouteHistory + PlatformProvider) plus the default RequestHandler
- Examples: `crates/core/src/daemon.rs:41`
- Pattern: Both the Tauri shell and the future headless daemon instantiate one. `DaemonHandler` in the Tauri shell wraps it in an `Arc` and overrides `RouteOpen` to add picker behavior; everything else delegates.

**ConfigStore:**
- Purpose: Thread-safe, clone-able handle to the on-disk config document
- Examples: `crates/core/src/config/store.rs:84`
- Pattern: `Arc<Mutex<State>>` inside; exposes `document()` (full clone), `with_document(f)` (lock-held closure for hot paths), `replace(doc, writer)`, `watch(on_change)`. Anti-echo via UUID token in `Meta.last_writer_token`.

**Layered Config Capability Architecture (from `add-config-capability` skill):**
Each new user-configurable feature must be wired through five layers in order:
1. Core schema (`crates/core/src/config/mod.rs` or `rules.rs`) — define the field with `#[serde(default)]`
2. Daemon command (`apps/desktop/src-tauri/src/commands/mod.rs` + `lib.rs` + `capabilities/default.json`)
3. Frontend wrapper (`apps/desktop/src/lib/ipc.ts`)
4. React page (`apps/desktop/src/pages/*.tsx`)
5. CLI subcommand (`crates/cli/src/main.rs`)

## Entry Points

**Deep-link URL event:**
- Location: `apps/desktop/src-tauri/src/lib.rs:170` (registered in `app.deep_link().on_open_url(...)`)
- Triggers: macOS sends `open https://...` Apple Event to LinkPilot (registered as default browser)
- Responsibilities: Builds `RoutingContext` with opener detection, delegates to `url_handler::dispatch_system_url`

**Tauri GUI commands:**
- Location: `apps/desktop/src-tauri/src/commands/mod.rs`
- Triggers: React frontend calls `invoke(...)` via `apps/desktop/src/lib/ipc.ts`
- Responsibilities: Config mutations, routing simulation (`route_evaluate`, `route_open`), browser inventory, platform operations

**IPC server:**
- Location: `crates/ipc/src/server.rs`, hosted by `apps/desktop/src-tauri/src/lib.rs:197`
- Triggers: `lpt` CLI connects to `linkpilot.sock`; future NMH bridge will do the same
- Responsibilities: Length-prefixed JSON framing; all verbs except `RouteOpen` handled by `DaemonRuntime`

**lpt CLI:**
- Location: `crates/cli/src/main.rs`
- Triggers: User runs `lpt <subcommand>`
- Responsibilities: Read-prefers-daemon (IPC), writes always local (atomic config rewrite)

## Architectural Constraints

- **Threading:** Tauri uses a single-threaded main thread for UI events; `dispatch::execute` spawns a detached `std::thread` for Ask flows to avoid deadlocking the main thread while waiting for picker resolution. The IPC server runs on a 2-worker Tokio runtime in a separate thread group.
- **Global state:** `AppState` in `apps/desktop/src-tauri/src/state.rs` is the Tauri-managed singleton. `picker::PickerState` is a separate Tauri-managed state. No module-level `static` mutable state.
- **Circular imports:** `linkpilot-core` must not depend on `linkpilot-ipc` (would create a cycle since `ipc` depends on `core`). `RequestHandler` trait lives in `core::daemon` for this reason, re-exported from `ipc::server`.
- **Config writes are always local:** Both the GUI and the CLI atomically rewrite the file on disk. A running daemon is notified via fsnotify. This means "daemon" does not mean "single authority" — any process can write the config and the daemon self-heals via the watcher.
- **macOS-only features gated by `#[cfg(target_os = "macos")]`:** AppKit-specific code in `picker.rs`, `tray.rs`, `platform-mac/` is never compiled on Linux/Windows. The workspace checks clean on all platforms via `StubProvider`.

## Anti-Patterns

### Calling `config_replace` from the frontend for a single-field change that has a side effect
**What happens:** Using `ipc.configReplace(doc)` to flip `settings.launch_at_login`
**Why it's wrong:** `config_replace` only writes JSON. The macOS LaunchAgent plist is never touched, so the toggle looks inert.
**Do this instead:** Use the dedicated `set_launch_at_login` command (`commands/mod.rs:141`) which calls `platform.autostart().set_enabled(enabled)` before writing the config.

### Adding a new IPC verb for something that is just a config field mutation
**What happens:** Adding `Request::SetMyFlag { ... }` + `Response::MyFlagAck` to `protocol.rs`
**Why it's wrong:** `ConfigGet` + `ConfigReplace` (or `rule_upsert`/`workspace_upsert`) handle arbitrary mutations. New IPC verbs add protocol version pressure and break older clients.
**Do this instead:** Add a dedicated Tauri command in `commands/mod.rs` that reads-modify-writes the `ConfigDocument`. Only add a new IPC verb when the daemon must perform side effects the client can't (platform handshake, in-memory state reads like `RouteHistory`).

### Omitting `#[serde(default)]` on a new ConfigDocument field
**What happens:** Adding a non-Option field without `#[serde(default = "fn")]`
**Why it's wrong:** Every config file written before this change fails to deserialize → users lose all their rules on upgrade.
**Do this instead:** Always mark new `ConfigDocument`/`Settings`/`Workspace`/`Rule` fields with `#[serde(default)]` or `#[serde(default = "default_fn")]`. Never bump `SCHEMA_VERSION` unless the shape change is truly breaking and migration code is provided.

### Adding a rule matcher without a corresponding `MatcherEval` branch
**What happens:** New `MatcherTree` variant added in `rules.rs` but `eval_tree` in `routing.rs` doesn't handle it
**Why it's wrong:** Rust non-exhaustive match error at compile time — but if somehow bridged, the rule silently never fires.
**Do this instead:** Add both the `MatcherTree` variant in `rules.rs` AND the `eval_tree` branch + `MatcherEval` variant in `routing.rs` in the same commit. See the `add-config-capability` skill at `.agents/skills/add-config-capability/SKILL.md`.

## Error Handling

**Strategy:** Fail visibly at every layer. Errors are surfaced as `Response::Error { code, message }` over IPC, `Err(String)` from Tauri commands (serialized to JS), `tracing::error!` for internal failures, and `app.emit("route-failed", ...)` for async launch failures.

**Patterns:**
- IPC server: unknown verbs return `Error { code: "unknown-verb" }` rather than dropping the connection (protocol v2 forward-compatibility)
- Dispatch: `open_with_default_fallback()` tries the rule's target, then the configured default, then falls back to Ask rather than silently swallowing a launch failure
- ConfigStore: parse errors in the fsnotify callback are logged and the in-memory doc is left unchanged — stale is better than empty
- CLI: `anyhow::Result` propagated to the top; human-readable messages printed to stderr

## Cross-Cutting Concerns

**Logging:** `tracing` + `tracing-subscriber` throughout. Log level controlled via `RUST_LOG` env var. Frontend uses the `route-logged` and `config-changed` Tauri events for real-time Inspector/Settings updates.
**Validation:** URL parsing via the `url` crate at every entry point. Bundle-id matching in `MatcherTree::SourceApp` prefers bundle id over display name (localization-safe). Config schema validated at load time via serde; migration for legacy `priority` field applied at first load.
**Authentication:** Not applicable — this is a local-only macOS app with no network server. Download verification for self-updates uses SHA-256 from `checksums.txt` before writing any file to disk.

---

*Architecture analysis: 2026-06-05*
