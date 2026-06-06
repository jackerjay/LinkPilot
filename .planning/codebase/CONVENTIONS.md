# Coding Conventions

**Analysis Date:** 2026-06-05

---

## Governing Contract (12 Rules — `CLAUDE.md` + `AGENTS.md`)

These rules override style preferences when they conflict. Every code contributor (human or AI) must follow them:

1. Think before coding. State assumptions. Surface tradeoffs.
2. Simplicity first. Minimum code that solves the problem. No speculative features.
3. Surgical changes. Touch only what is asked. Match existing style.
4. Goal-driven execution. Define success criteria and verify.
5. Don't make the model do non-language work (retry, routing, thresholds → deterministic code).
6. Hard token budgets, no exceptions.
7. Surface conflicts, do not average them.
8. Read before you write (file + nearby siblings first).
9. Tests are required but are not the goal. Tests must check behavior.
10. Long-running operations require checkpoints.
11. **Convention beats novelty.** Match the existing pattern even if a "better" one exists.
12. **Fail visibly, not silently.** Surface every skipped record, rollback, constraint violation.

---

## Rust Conventions

### Naming

**Files:** `snake_case.rs` (e.g. `config/store.rs`, `platform.rs`, `app_icon.rs`)

**Types / Enums / Traits:** `PascalCase` (e.g. `ConfigDocument`, `MatcherTree`, `PlatformProvider`, `MacProvider`)

**Functions / Methods:** `snake_case` (e.g. `load_or_init`, `evaluate_explained`, `default_config_path`)

**Constants:** `SCREAMING_SNAKE_CASE` (e.g. `DEFAULT_CAPACITY`, `ERROR_UNKNOWN_VERB`)

**Modules:** `snake_case`, organized by domain — not by type (e.g. `browser`, `routing`, `rules`, `platform`)

### Module-Level Doc Comments

Every `lib.rs` and every module file with public API carries a `//!` doc comment explaining its purpose and constraints. Pure functions are called out explicitly:

```rust
// crates/core/src/routing.rs
//! Router: turns a [`RoutingContext`] into a [`RoutingDecision`] given the
//! current [`ConfigDocument`]. Pure function over data — no IO.
```

### Serde Wire Format

All public serializable enums use **tagged variants with `rename_all = "kebab-case"`**. This is the cross-language wire contract — do not deviate:

```rust
// Discriminant tags:
#[serde(tag = "op",     rename_all = "kebab-case")]   // MatcherTree, MatcherEval
#[serde(tag = "kind",   rename_all = "kebab-case")]   // Action
#[serde(tag = "action", rename_all = "kebab-case")]   // RoutingDecision
#[serde(tag = "type",   rename_all = "kebab-case")]   // IPC Request / Response
```

Struct fields use **`snake_case`** by default (serde default), matching the JSON on disk.

Language DSL (TypeScript) maps camelCase → snake_case via `compile()`. The Rust side only ever sees snake_case.

### Platform Gating

Platform-specific code is always gated with `#[cfg(target_os = ...)]`, never with runtime checks:

```rust
// crates/core/src/config/store.rs
#[cfg(target_os = "macos")]
{ /* macOS path */ }

#[cfg(target_os = "linux")]
{ /* XDG path */ }

#[cfg(target_os = "windows")]
{ /* APPDATA path */ }
```

The `crates/platform-mac/src/lib.rs` file-level gate is `#![cfg(target_os = "macos")]`. Stub crates (`platform-win`, `platform-linux`) simply re-export `StubProvider`:

```rust
// crates/platform-win/src/lib.rs
#![cfg(target_os = "windows")]
pub use linkpilot_core::platform::StubProvider as WinProvider;
```

AppKit-specific code (tray, picker) in `apps/desktop/src-tauri/src/` uses `objc2` + `window-vibrancy`; these modules are only compiled on macOS because Cargo gating at the crate level handles it.

### Error Handling

- `thiserror` for typed, structured errors in library crates (e.g. `ConfigError` in `crates/core/src/config/store.rs`)
- `anyhow` for binary entrypoint error propagation (`crates/cli/src/main.rs`, `crates/headless-daemon/src/main.rs`)
- Tauri commands return `Result<T, String>` — `.map_err(|e| e.to_string())` at the boundary
- `.expect("mutex poisoned")` is acceptable for `Mutex::lock()` — panicking on poisoned mutex is intentional because a poisoned lock means a previous thread panicked mid-write (the program state is unknown)
- `.unwrap()` in tests is conventional — no try/catch in test code
- Never silently swallow errors. Log or surface them

```rust
// Tauri command error pattern (commands/mod.rs)
pub fn rule_upsert(state: State<'_, AppState>, rule: Rule) -> Result<(), String> {
    state.config.replace(doc, WriterId::Gui).map_err(|e| e.to_string())
}
```

### Immutability

- Prefer returning new values over mutating in place
- `config_replace` takes a full `ConfigDocument` snapshot, not a diff — the mutator pattern is: get document clone → mutate clone → replace whole document
- `Router` is a zero-cost wrapper that borrows `&ConfigDocument` — stateless; no `&mut self`

### Workspace Version Discipline

Workspace `Cargo.toml` defines the canonical version (`0.5.6`) and shared deps. All crates use `{ workspace = true }` for shared deps — do not add duplicate version pins in individual `Cargo.toml` files.

---

## TypeScript / React Conventions

### Naming

**Files:**
- React components: `PascalCase.tsx` (e.g. `RuleEditor.tsx`, `HaloShell.tsx`)
- Pages: `kebab-case.tsx` (e.g. `rules.tsx`, `test-url.tsx`, `menu-bar.tsx`)
- Library modules: `camelCase.ts` (e.g. `ipc.ts`, `types.ts`, `utils.ts`)
- Type-only geometry: `camelCase.ts` (e.g. `geometry.ts`, `types.ts` inside feature dirs)

**Functions / Hooks:** `camelCase` (e.g. `newRule`, `appPathFromExecutable`)

**React Components:** `PascalCase` function (not `React.FC`) — explicit props interface

**Types / Interfaces:** `PascalCase` (e.g. `BrowserTarget`, `RouteRequest`, `EditorState`)

**Constants:** `SCREAMING_SNAKE_CASE` for sentinels (e.g. `NO_WORKSPACE = "__none__"`)

### TypeScript Strictness

`tsconfig.json` enforces `strict: true`, `noUnusedLocals: true`, `noUnusedParameters: true`, `noImplicitAny: true`, `noImplicitReturns: true`.

- Never use `any`
- Props are typed with named `interface` or `type`, never inline
- Callbacks are explicitly typed (e.g. `onSave: (rule: Rule) => Promise<void>`)
- Type unions over enums:

```typescript
type TabId = "menu-bar" | "rules" | "test-url" | "inspector" | "browsers" | "settings";
type EditorState = { kind: "closed" } | { kind: "new" } | { kind: "edit"; rule: Rule };
```

### Import Organization

1. React core imports (`react`, `react-dom`)
2. External libraries (`@tauri-apps/api`, `react-i18next`, `lucide-react`)
3. Internal `@/components/...` (via `@` alias for `./src`)
4. Internal `@/lib/...`
5. Types (`import type { ... } from "@/lib/types"`)

Path alias `@` maps to `apps/desktop/src/` (configured in `vite.config.ts` and `tsconfig.json`).

### State and Data Flow

The canonical data flow is:

```
React component
  → ipc.someCommand(args)        # apps/desktop/src/lib/ipc.ts
  → invoke<T>("command_name")    # @tauri-apps/api/core
  → #[tauri::command] fn         # apps/desktop/src-tauri/src/commands/mod.rs
  → ConfigStore / PlatformProvider / RouteHistory
```

Event flow (daemon → frontend):
```
Tauri Emitter → "route-logged" / "config-changed" events
  → onRouteLogged / onConfigChanged listeners in ipc.ts
  → React state update
```

`lib/ipc.ts` is the **only** file that calls `invoke`. Pages and components never call `invoke` directly.

Types in `lib/types.ts` mirror Rust structs from `linkpilot-core`. The file has an explicit comment: "Kept narrow on purpose — the frontend only needs the fields it renders." Do not add fields that the frontend doesn't render.

### Component Design

- Named function exports (not default exports for pages — allows tree-shaking)
- Props interface defined immediately above the component function
- No `React.FC` annotation
- `useState` / `useEffect` / `useCallback` — functional components throughout
- Lazy loading for heavy dependencies (CodeMirror via `lazy()`): `apps/desktop/src/pages/rules.tsx`

### Styling

- Tailwind v4 utility classes
- `cn()` helper from `lib/utils.ts` for conditional class merging (wraps `clsx` + `tailwind-merge`)
- Radix UI primitives (`@radix-ui/react-*`) for interactive elements (Select, Checkbox, Tooltip, Dialog, Label)
- `lucide-react` for all icons
- No inline `style={}` objects — only Tailwind classes

---

## Commit Message Conventions

Format (conventional commits):

```
<type>: <description>

<optional body>
```

Types in use: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`, `release`

Examples from recent history:

```
fix(homebrew): daemon resource URL rendered v/ (404) — capture release tag
release: fill v0.5.6 Homebrew shas, flip READMEs to brew install
ci: migrate Actions off deprecated Node 20 → latest majors (#42)
chore(deps): bump uuid 1.23.1 → 1.23.2 (#39)
```

Scopes are used when the change is isolated to a subsystem (e.g. `(homebrew)`, `(deps)`). Body is optional but used for complex fixes. No Co-Authored-By attribution (globally disabled).

---

## Immutability and Functional Style Preferences

- Prefer immutable patterns: cloning the document before mutation, returning new collections
- `iter().map().collect()` over in-place mutation
- `Router` is a pure function wrapper — `evaluate()` takes `&self` and `&RoutingContext`, returns owned `RoutingDecision`
- Frontend: spread operators for React state updates, never direct mutation of state objects

---

## UI Interaction Rules (from `CLAUDE.md`)

These are hard constraints on the picker/profile-halo UI:

- Profile Halo controls **must stay attached to the wheel's actual sectors**: add → `+` sector; delete → near active sector; reorder → direct sector drag
- Halo drag editing **must show real drag state**: active sector highlight + pointer-following ghost + live slot reflow while dragging
- No detached side panels or global controls when the action target is ambiguous

---

## Anti-Patterns to Avoid

**Half-wired capabilities:** When extending `ConfigDocument` (new matcher, action, settings field), every layer must be updated in the same commit: core schema → Tauri command → `lib/ipc.ts` wrapper → React page → `lpt` CLI subcommand. See `.claude/skills/add-config-capability/SKILL.md` for the full checklist.

**New IPC `Request`/`Response` variant for a config field:** Almost never needed. `config_get` + `config_replace` (or `rule_upsert` / `workspace_upsert`) already handle arbitrary doc mutations. Add a dedicated verb only when the daemon does platform work the client can't.

**`#[serde(default)]` missing on new fields:** Old configs on disk won't deserialize. Always add `#[serde(default)]` or `#[serde(default = "fn")]` on any new optional or added-later field in `ConfigDocument` or its children.

**`invoke()` called outside `lib/ipc.ts`:** The entire Tauri command surface must be accessed through `ipc.ts` typed wrappers only.

---

*Convention analysis: 2026-06-05*
