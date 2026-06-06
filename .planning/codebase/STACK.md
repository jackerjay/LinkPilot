# Technology Stack

**Analysis Date:** 2026-06-05

## Languages

**Primary:**
- Rust — Entire backend: workspace crates, Tauri shell, daemon, CLI, NMH
- TypeScript (strict) — Tauri frontend in `apps/desktop/src/`

**Secondary:**
- Shell (bash) — `apps/desktop/scripts/patch-info-plist.sh`, release DMG packaging in CI
- Ruby — Homebrew formulae/casks in `packaging/homebrew/`

## Runtime

**Environment:**
- macOS 12.0+ (Monterey) — production target; `tauri.conf.json` `minimumSystemVersion: "12.0"`
- Linux — workspace-check only (no Tauri shell; `platform-linux` is a stub)
- Windows — workspace-check only (no Tauri shell; `platform-win` is a stub)

**Rust Toolchain:**
- `rust-version = "1.80"` (MSRV, enforced in `Cargo.toml`)
- Edition: `2021`
- Channel: stable (CI uses `dtolnay/rust-toolchain@stable`)

**Node:**
- Version: 22 (pinned in CI `actions/setup-node@v6 node-version: 22`)
- Package Manager (desktop): yarn via corepack (`corepack yarn install`)
- Package Manager (config-dsl): bun 1.3 (`packages/config-dsl/bun.lock`)

## Frameworks

**Core Rust:**
- `tokio` 1 (features: `rt-multi-thread`, `macros`, `sync`, `io-util`, `net`, `time`) — async runtime for IPC server/client

**Desktop Shell:**
- `tauri` 2 (features: `tray-icon`, `image-png`, `macos-private-api`) — `apps/desktop/src-tauri/`
- `tauri-plugin-deep-link` 2 — registers `http`/`https` URL schemes, fires `on_open_url` events
- `tauri-plugin-opener` 2 — opens external URLs from the frontend

**Frontend:**
- React 18.3.x — `apps/desktop/src/`
- Vite 5.3.x — build tool, dev server on port 5173; config `apps/desktop/vite.config.ts`
- Tailwind CSS v4 (`@tailwindcss/vite` 4.3) — utility-first styling
- Radix UI primitives — accessible UI components (`@radix-ui/react-*`)
- i18next + react-i18next — internationalization; locales: `en`, `zh-CN`, `zh-TW`, `ja-JP`

**Testing (config-dsl):**
- bun test — `packages/config-dsl/` unit tests, run in `npm-publish.yml`

## Key Dependencies

**Critical Rust (workspace-level):**
- `serde` 1 + `serde_json` 1 — JSON serialization for IPC protocol, config file, Tauri commands
- `thiserror` 1 — structured error types across all crates
- `anyhow` 1 — ergonomic error propagation in binaries (daemon, CLI, desktop)
- `uuid` 1 (features: `v4`, `serde`) — rule/workspace/browser IDs
- `url` 2 (features: `serde`) — URL parsing for routing evaluation
- `clap` 4 (features: `derive`) — CLI argument parsing in `lpt` and `linkpilot-daemon`
- `tracing` 0.1 + `tracing-subscriber` 0.3 (features: `env-filter`) — structured logging
- `notify` 6 — filesystem watcher for config file live-reload (fsnotify)

**macOS-specific Rust:**
- `core-foundation` 0.10 + `core-foundation-sys` 0.8 — LaunchServices FFI for `LSSetDefaultHandlerForURLScheme` / `LSCopyDefaultHandlerForURLScheme`
- `objc2` 0.5 — raw Objective-C messaging for NSWindow / AppKit calls in picker and tray
- `objc2-foundation` 0.3 (features: `NSString`) — Foundation types
- `objc2-app-kit` 0.2 (features: `NSWorkspace`, `NSRunningApplication`) — browser discovery and app activation
- `window-vibrancy` 0.6 — translucent frosted-glass background for picker window
- `libc` 0.2 — `kill(pid, 0)` for PID-file liveness probing (core, unix target); `setsid()` for daemon detach (CLI, macOS target)

**Desktop Tauri shell:**
- `base64` 0.22 — PNG app icon encoding as data URLs for the frontend

**Frontend:**
- `@tauri-apps/api` 2 — `invoke`, `listen`, `event` bridge from JS to Rust commands
- `@codemirror/lang-json` 6 + `@uiw/react-codemirror` 4 — JSON editor for advanced config editing page
- `lucide-react` 1 — icon set
- `class-variance-authority` + `clsx` + `tailwind-merge` — class composition utilities
- `tailwindcss-animate` — CSS animation utilities

**config-dsl (`packages/config-dsl`):**
- TypeScript 5.5 — compiled to ESM, no runtime dependencies; pure type/DSL layer

**Tools:**
- `image` 0.25 (features: `png`) — `tools/icon-padder` for brand icon canvas padding

## Configuration

**Environment:**
- No `.env` files or secrets required for development (macOS + Rust + Node is sufficient)
- `RUST_LOG` / `RUST_LOG=info,linkpilot=debug` — controls `tracing-subscriber` filter at runtime
- `TAURI_DEV_HOST` — optional Vite dev server host override for remote dev

**Build configuration files:**
- `Cargo.toml` — workspace root; defines versions, editions, shared deps, release profile (`lto="thin"`, `codegen-units=1`, `strip=true`)
- `apps/desktop/src-tauri/Cargo.toml` — desktop crate; Tauri features and per-platform deps
- `apps/desktop/src-tauri/tauri.conf.json` — Tauri bundler config; app identifier `app.linkpilot.desktop`, window dimensions, deep-link schemes, macOS `minimumSystemVersion: "12.0"`
- `apps/desktop/vite.config.ts` — Vite plugins (react, tailwindcss), `@` path alias, dev server port 5173
- `apps/desktop/tsconfig.json` — `strict: true`, `noUnusedLocals`, `noUnusedParameters`, target ES2020, `@/*` alias

**Release profile** (`Cargo.toml`):
```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```

## Platform Requirements

**Development (macOS):**
- Rust stable toolchain (MSRV 1.80)
- Node 22
- yarn (via corepack)
- Command: `npx tauri dev` from `apps/desktop/`

**Development (Linux — workspace check only):**
- `libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev librsvg2-dev libssl-dev pkg-config`
- Cannot run Tauri shell

**Production:**
- macOS 12.0+ (Monterey) — enforced by `tauri.conf.json` and Homebrew cask `depends_on macos: :monterey`
- Architecture: `aarch64-apple-darwin` or `x86_64-apple-darwin` (separate per-arch DMGs; no universal binary)
- Unsigned/unnotarized — requires `xattr -dr com.apple.quarantine` or Homebrew cask postflight

---

*Stack analysis: 2026-06-05*
