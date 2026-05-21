//! `lpt` — the LinkPilot command-line client.
//!
//! Read operations prefer the running daemon (via IPC) so they reflect the
//! daemon's in-memory snapshot; on offline daemon they fall back to reading
//! the on-disk config + platform inventory directly. Write operations always
//! mutate the on-disk config file atomically — the daemon's fsnotify watcher
//! (with its anti-echo token) picks them up and reloads, so a running GUI
//! refreshes within a frame.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use linkpilot_core::browser::{
    BrowserId, BrowserKind, BrowserProfile, BrowserTarget, InstalledBrowser,
};
use linkpilot_core::config::{
    default_config_path, ConfigDocument, ConfigStore, Workspace, WriterId,
};
use linkpilot_core::history::RouteRecord;
use linkpilot_core::platform::{PlatformProvider, SetDefaultOutcome};
use linkpilot_core::protocol::{DoctorReport, Request, Response, ERROR_UNKNOWN_VERB};
use linkpilot_core::routing::{Router, RoutingContext, RoutingDecision, Source, SourceKind};
use linkpilot_core::rules::{Action, MatcherTree, Rule, RuleId, RuleSource};
use linkpilot_ipc::client::{self, ClientError};
use linkpilot_ipc::path::default_endpoint;
use linkpilot_ipc::transport::TransportError;

// ---------------------------------------------------------------------------
// Clap structures

#[derive(Parser, Debug)]
#[command(name = "lpt", version, about = "LinkPilot command-line client")]
struct Cli {
    /// Override the config file path (defaults to the platform location).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Skip the daemon when reading, and always read/write locally.
    /// Writes are always local even without this flag; it only affects reads.
    #[arg(long, global = true)]
    local: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Open a URL through the LinkPilot router.
    Open {
        url: String,
        /// Pretend the URL came from this application (sets `source.app_name`).
        #[arg(long)]
        from_app: Option<String>,
        /// Bundle id paired with `--from-app` for stable cross-locale matching.
        #[arg(long)]
        from_app_bundle_id: Option<String>,
        /// Simulate a route arriving from this browser id (extension-shaped).
        #[arg(long)]
        from_browser: Option<String>,
        /// Source profile id within `--from-browser`.
        #[arg(long)]
        from_profile: Option<String>,
        /// Show the decision without launching anything.
        #[arg(long)]
        dry_run: bool,
    },

    /// Diagnose default-browser, config, and installed-browser state.
    Doctor {
        /// Emit the report as JSON to stdout instead of a human summary.
        #[arg(long)]
        json: bool,
    },

    /// Inspect or modify rules.
    Rules {
        #[command(subcommand)]
        action: RulesAction,
    },

    /// Inspect or modify workspaces.
    #[command(alias = "ws")]
    Workspaces {
        #[command(subcommand)]
        action: WorkspacesAction,
    },

    /// Inspect, import, or export the config document.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Inspect or modify daemon-level settings (smart-routing, autostart, …).
    Settings {
        #[command(subcommand)]
        action: SettingsAction,
    },

    /// List installed/custom browsers, profiles, and manage custom entries.
    Browsers {
        #[command(subcommand)]
        action: BrowsersAction,
    },

    /// Query or set LinkPilot's "is-default-browser" registration.
    #[command(alias = "default")]
    DefaultBrowser {
        #[command(subcommand)]
        action: DefaultBrowserAction,
    },

    /// Manage the background `linkpilot-daemon` — start, stop, status,
    /// install/uninstall the LaunchAgent, tail logs.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Show recent routing decisions from the daemon's in-memory history.
    /// Requires a running daemon (protocol v2+); v0.1 daemons can't
    /// answer this and the CLI prints an upgrade hint.
    #[command(alias = "hist")]
    History {
        /// Cap on records returned (default: 50). The daemon's buffer
        /// holds up to 1000 entries.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Emit each record as a single JSON object per line (jq-friendly).
        #[arg(long)]
        json: bool,
    },
}

// --- rules ---

#[derive(Subcommand, Debug)]
enum RulesAction {
    /// Print every rule in the config in list order (top wins).
    List {
        /// Emit each rule as JSON on stdout (one rule per line).
        #[arg(long)]
        json: bool,
        /// Include disabled rules (default: show only enabled).
        #[arg(long)]
        all: bool,
    },
    /// Print one rule's full JSON.
    Show { id: String },
    /// Create a new rule. Inserted at the bottom of the list (lowest
    /// priority); use `lpt rules move` to raise it.
    Add(Box<RuleAddArgs>),
    /// Delete a rule by id (full or prefix; prefix must be unambiguous).
    Delete { id: String },
    /// Mark a rule enabled.
    Enable { id: String },
    /// Mark a rule disabled.
    Disable { id: String },
    /// Reorder a rule in the priority list. List order IS priority —
    /// top of the list wins.
    Move {
        id: String,
        #[arg(value_enum)]
        position: MovePosition,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum MovePosition {
    /// Move to the very top (highest priority).
    Top,
    /// Move one slot up (toward higher priority).
    Up,
    /// Move one slot down (toward lower priority).
    Down,
    /// Move to the very bottom (lowest priority).
    Bottom,
}

#[derive(clap::Args, Debug)]
struct RuleAddArgs {
    /// Start the rule disabled.
    #[arg(long)]
    disabled: bool,
    /// Free-form note shown in the rules list.
    #[arg(long)]
    note: Option<String>,
    /// Workspace id this rule belongs to (toggle the workspace to batch-disable).
    #[arg(long)]
    workspace: Option<String>,

    // -- Matcher (combine into AND when multiple given; or use --when-json) --
    /// Glob host match (e.g. `github.com`, `*.figma.com`).
    #[arg(long)]
    host: Option<String>,
    /// Glob path match (e.g. `/oauth/*`).
    #[arg(long)]
    path: Option<String>,
    /// Source-app name match (case-insensitive).
    #[arg(long)]
    from_app: Option<String>,
    /// macOS bundle id for `--from-app` (stable across locale + display-name drift).
    #[arg(long, requires = "from_app")]
    from_app_bundle_id: Option<String>,
    /// Source-browser id (when navigation came from a browser extension).
    #[arg(long)]
    from_browser: Option<String>,
    /// Source profile id within `--from-browser`.
    #[arg(long)]
    from_profile: Option<String>,
    /// Raw MatcherTree JSON — overrides every `--host`/`--path`/etc.
    #[arg(long, value_name = "JSON")]
    when_json: Option<String>,

    // -- Action (exactly one required, unless --then-json is given) --
    /// Open URL in this browser id (e.g. `chrome`, `arc`, `safari`).
    #[arg(long)]
    target: Option<String>,
    /// Profile within `--target`.
    #[arg(long, requires = "target")]
    target_profile: Option<String>,
    /// Open the target in incognito / private mode.
    #[arg(long, requires = "target")]
    incognito: bool,
    /// Force a new window.
    #[arg(long, requires = "target")]
    new_window: bool,
    /// Keep navigation in the source browser (no handoff).
    #[arg(long, conflicts_with_all = ["target", "ask", "block"])]
    keep_source: bool,
    /// Pop the browser picker for this URL.
    #[arg(long, conflicts_with_all = ["target", "keep_source", "block"])]
    ask: bool,
    /// Drop the URL silently.
    #[arg(long, conflicts_with_all = ["target", "keep_source", "ask"])]
    block: bool,
    /// Raw Action JSON — overrides every `--target`/`--ask`/etc.
    #[arg(long, value_name = "JSON")]
    then_json: Option<String>,
}

// --- workspaces ---

#[derive(Subcommand, Debug)]
enum WorkspacesAction {
    /// List every workspace and whether it's enabled.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create or update a workspace.
    Add {
        /// Stable id referenced by rules (`workspace_id`).
        id: String,
        /// Display name shown in the GUI.
        #[arg(long)]
        name: String,
        /// Optional description.
        #[arg(long)]
        description: Option<String>,
        /// Start the workspace disabled.
        #[arg(long)]
        disabled: bool,
    },
    /// Delete a workspace. Rules pointing at it revert to "ungrouped".
    Delete { id: String },
    /// Enable a workspace (its rules participate in routing again).
    Enable { id: String },
    /// Disable a workspace (skips every rule whose `workspace_id` matches).
    Disable { id: String },
}

// --- config ---

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Print the whole config document as pretty-printed JSON.
    Show,
    /// Print the resolved config file path.
    Path,
    /// Replace the on-disk config with the JSON at PATH.
    Import { path: PathBuf },
    /// Write the current config as pretty-printed JSON to PATH.
    Export { path: PathBuf },
    /// Change the fallback target used when no rule matches.
    SetDefaultTarget {
        /// Browser id (e.g. `chrome`, `arc`, `safari`).
        browser: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        incognito: bool,
        #[arg(long)]
        new_window: bool,
    },
    /// Compile a `linkpilot.config.ts` (via @linkpilot/config DSL) into
    /// the daemon's JSON config. Requires `bun` on PATH.
    Compile {
        /// Path to the `.ts` config file. Should `import { ... } from
        /// "@linkpilot/config"` and call `printConfig(defineConfig({...}))`.
        source: PathBuf,
        /// Write the compiled JSON to this path instead of replacing the
        /// daemon's active config. Useful for `git`-tracked outputs or
        /// dry-runs.
        #[arg(long)]
        to: Option<PathBuf>,
    },
}

// --- settings ---

#[derive(Subcommand, Debug)]
enum SettingsAction {
    /// Print all settings as JSON.
    Show,
    /// Toggle the master rule-evaluation kill-switch.
    SmartRouting { value: OnOff },
    /// Toggle the LaunchAgent that opens LinkPilot at login.
    /// NOTE: only the config flag is flipped; the actual LaunchAgent plist
    /// install/uninstall is owned by the GUI and currently CLI-only flips
    /// the persisted preference.
    LaunchAtLogin { value: OnOff },
    /// Toggle startup checks for newer GitHub Release builds.
    AutoUpdates { value: OnOff },
    /// Toggle whether URL query strings are kept in route history.
    RecordQueryStrings { value: OnOff },
    /// Set history retention in days, or `clear` to keep forever.
    HistoryRetention { value: String },
    /// Set the visual style of the browser+profile picker wheel.
    /// One of `frosted` (default), `bezel`, or `crown`.
    PickerStyle { value: String },
    /// Set the UI display language. One of `system` (default, follow OS),
    /// `en`, `zh-CN`, `zh-TW`, `ja-JP`.
    Language { value: String },
}

#[derive(Clone, Copy, Debug)]
enum OnOff {
    On,
    Off,
}
impl std::str::FromStr for OnOff {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "on" | "true" | "yes" | "1" => Ok(OnOff::On),
            "off" | "false" | "no" | "0" => Ok(OnOff::Off),
            other => Err(format!("expected on/off, got {other}")),
        }
    }
}
impl OnOff {
    fn as_bool(self) -> bool {
        matches!(self, OnOff::On)
    }
}

// --- browsers ---

#[derive(Subcommand, Debug)]
enum BrowsersAction {
    /// List installed (auto-detected) + custom browsers.
    List {
        #[arg(long)]
        json: bool,
        /// Hide custom browsers; show only what the inventory auto-detected.
        #[arg(long)]
        installed_only: bool,
    },
    /// List profiles for one browser id.
    Profiles {
        browser: String,
        #[arg(long)]
        json: bool,
    },
    /// Manage user-added (custom) browser entries.
    Custom {
        #[command(subcommand)]
        action: CustomBrowserAction,
    },
}

#[derive(Subcommand, Debug)]
enum CustomBrowserAction {
    /// Add or update a custom browser entry.
    Add {
        /// Stable id (matches what rules reference).
        #[arg(long)]
        id: String,
        /// Display name.
        #[arg(long)]
        name: String,
        /// Engine family.
        #[arg(long, value_parser = parse_kind)]
        kind: BrowserKind,
        /// Absolute path to the executable (or .app bundle on macOS).
        #[arg(long, value_name = "PATH")]
        exec: PathBuf,
        /// macOS bundle id / Windows AppUserModelID.
        #[arg(long)]
        bundle_id: Option<String>,
    },
    /// Remove a custom browser by id.
    Remove { id: String },
}

fn parse_kind(s: &str) -> std::result::Result<BrowserKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "chromium" => Ok(BrowserKind::Chromium),
        "firefox" => Ok(BrowserKind::Firefox),
        "safari" => Ok(BrowserKind::Safari),
        "arc" => Ok(BrowserKind::Arc),
        "unknown" => Ok(BrowserKind::Unknown),
        other => Err(format!(
            "unknown kind '{other}' — expected chromium|firefox|safari|arc|unknown"
        )),
    }
}

// --- default browser ---

#[derive(Subcommand, Debug)]
enum DefaultBrowserAction {
    /// Is LinkPilot currently registered as the system default?
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Ask the OS to make LinkPilot the default browser.
    /// macOS pops a system confirmation; Windows opens Settings → Default apps.
    Set,
}

// --- daemon (M2) ---

#[derive(Subcommand, Debug)]
enum DaemonAction {
    /// Spawn a background `linkpilot-daemon --serve` if none is running.
    /// Idempotent — does nothing if a daemon already answers StatePing.
    Start,
    /// Send SIGTERM to the running daemon via the PID file. Refuses if
    /// the daemon is managed by launchd (run `lpt daemon uninstall` instead).
    Stop,
    /// Stop, wait for the socket to close, then start again.
    Restart,
    /// Print daemon liveness, version, PID, socket path, and whether
    /// the LaunchAgent is installed.
    Status {
        /// JSON output for scripting.
        #[arg(long)]
        json: bool,
    },
    /// Install the LaunchAgent plist so the daemon runs at login.
    /// Idempotent: re-installing overwrites the plist and reloads launchd.
    Install,
    /// Unload + delete the LaunchAgent plist, then SIGTERM the daemon.
    Uninstall,
    /// Tail `~/Library/Logs/LinkPilot/daemon.{out,err}.log`.
    Logs {
        /// Follow the log file as new lines are appended (Ctrl-C to exit).
        #[arg(long, short = 'f')]
        follow: bool,
        /// Number of trailing lines to print before following.
        #[arg(long, short = 'n', default_value = "50")]
        lines: usize,
    },
}

// ---------------------------------------------------------------------------
// Platform glue + main

#[cfg(target_os = "macos")]
fn make_platform() -> Box<dyn PlatformProvider> {
    Box::new(linkpilot_platform_mac::MacProvider::default())
}

#[cfg(not(target_os = "macos"))]
fn make_platform() -> Box<dyn PlatformProvider> {
    Box::new(linkpilot_core::platform::StubProvider)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Open {
            url,
            from_app,
            from_app_bundle_id,
            from_browser,
            from_profile,
            dry_run,
        } => run_open(
            cli.config,
            cli.local,
            url,
            from_app,
            from_app_bundle_id,
            from_browser,
            from_profile,
            dry_run,
        ),
        Command::Doctor { json } => run_doctor(cli.config, cli.local, json),
        Command::Rules { action } => match action {
            RulesAction::List { json, all } => run_rules_list(cli.config, cli.local, json, all),
            RulesAction::Show { id } => run_rules_show(cli.config, cli.local, &id),
            RulesAction::Add(args) => run_rules_add(cli.config, args),
            RulesAction::Delete { id } => run_rules_delete(cli.config, &id),
            RulesAction::Enable { id } => run_rules_set_enabled(cli.config, &id, true),
            RulesAction::Disable { id } => run_rules_set_enabled(cli.config, &id, false),
            RulesAction::Move { id, position } => run_rules_move(cli.config, &id, position),
        },
        Command::Workspaces { action } => match action {
            WorkspacesAction::List { json } => run_workspaces_list(cli.config, cli.local, json),
            WorkspacesAction::Add {
                id,
                name,
                description,
                disabled,
            } => run_workspaces_add(cli.config, id, name, description, !disabled),
            WorkspacesAction::Delete { id } => run_workspaces_delete(cli.config, &id),
            WorkspacesAction::Enable { id } => run_workspaces_set_enabled(cli.config, &id, true),
            WorkspacesAction::Disable { id } => run_workspaces_set_enabled(cli.config, &id, false),
        },
        Command::Config { action } => match action {
            ConfigAction::Show => run_config_show(cli.config, cli.local),
            ConfigAction::Path => run_config_path(cli.config),
            ConfigAction::Import { path } => run_config_import(cli.config, path),
            ConfigAction::Export { path } => run_config_export(cli.config, cli.local, path),
            ConfigAction::SetDefaultTarget {
                browser,
                profile,
                incognito,
                new_window,
            } => run_config_set_default_target(cli.config, browser, profile, incognito, new_window),
            ConfigAction::Compile { source, to } => run_config_compile(cli.config, source, to),
        },
        Command::Settings { action } => match action {
            SettingsAction::Show => run_settings_show(cli.config, cli.local),
            SettingsAction::SmartRouting { value } => {
                run_settings_set(cli.config, |s| s.smart_routing_enabled = value.as_bool())
            }
            SettingsAction::LaunchAtLogin { value } => {
                run_settings_set(cli.config, |s| s.launch_at_login = value.as_bool())
            }
            SettingsAction::AutoUpdates { value } => {
                run_settings_set(cli.config, |s| s.auto_check_updates = value.as_bool())
            }
            SettingsAction::RecordQueryStrings { value } => {
                run_settings_set(cli.config, |s| s.record_query_strings = value.as_bool())
            }
            SettingsAction::HistoryRetention { value } => {
                let parsed = if value.eq_ignore_ascii_case("clear") || value == "0" {
                    None
                } else {
                    Some(
                        value
                            .parse::<u32>()
                            .with_context(|| format!("parsing days '{value}'"))?,
                    )
                };
                run_settings_set(cli.config, |s| s.history_retention_days = parsed)
            }
            SettingsAction::PickerStyle { value } => {
                let style = match value.to_ascii_lowercase().as_str() {
                    "frosted" => linkpilot_core::config::PickerStyle::Frosted,
                    "bezel" => linkpilot_core::config::PickerStyle::Bezel,
                    "crown" => linkpilot_core::config::PickerStyle::Crown,
                    other => {
                        bail!("unknown picker style '{other}' (expected frosted | bezel | crown)")
                    }
                };
                run_settings_set(cli.config, |s| s.picker_style = style)
            }
            SettingsAction::Language { value } => {
                // Match the on-disk kebab-case serde form so `lpt settings
                // language zh-CN` writes the same string the GUI reads back.
                let lang = match value.as_str() {
                    "system" => linkpilot_core::config::LanguagePref::System,
                    "en" | "en-US" | "en-us" => linkpilot_core::config::LanguagePref::En,
                    "zh-CN" | "zh-cn" => linkpilot_core::config::LanguagePref::ZhCn,
                    "zh-TW" | "zh-tw" => linkpilot_core::config::LanguagePref::ZhTw,
                    "ja-JP" | "ja-jp" | "ja" => linkpilot_core::config::LanguagePref::JaJp,
                    other => bail!(
                        "unknown language '{other}' (expected system | en | zh-CN | zh-TW | ja-JP)"
                    ),
                };
                run_settings_set(cli.config, |s| s.language = lang)
            }
        },
        Command::Browsers { action } => match action {
            BrowsersAction::List {
                json,
                installed_only,
            } => run_browsers_list(cli.config, cli.local, json, installed_only),
            BrowsersAction::Profiles { browser, json } => run_browsers_profiles(&browser, json),
            BrowsersAction::Custom { action } => match action {
                CustomBrowserAction::Add {
                    id,
                    name,
                    kind,
                    exec,
                    bundle_id,
                } => run_browsers_custom_add(cli.config, id, name, kind, exec, bundle_id),
                CustomBrowserAction::Remove { id } => run_browsers_custom_remove(cli.config, &id),
            },
        },
        Command::DefaultBrowser { action } => match action {
            DefaultBrowserAction::Status { json } => run_default_browser_status(json),
            DefaultBrowserAction::Set => run_default_browser_set(),
        },
        Command::Daemon { action } => match action {
            DaemonAction::Start => run_daemon_start(),
            DaemonAction::Stop => run_daemon_stop(),
            DaemonAction::Restart => run_daemon_restart(),
            DaemonAction::Status { json } => run_daemon_status(json),
            DaemonAction::Install => run_daemon_install(),
            DaemonAction::Uninstall => run_daemon_uninstall(),
            DaemonAction::Logs { follow, lines } => run_daemon_logs(follow, lines),
        },
        Command::History { limit, json } => run_history(limit, json),
    }
}

// ===========================================================================
// `lpt open`

#[allow(clippy::too_many_arguments)]
fn run_open(
    config: Option<PathBuf>,
    local: bool,
    url: String,
    from_app: Option<String>,
    from_app_bundle_id: Option<String>,
    from_browser: Option<String>,
    from_profile: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let context = build_context(
        &url,
        from_app,
        from_app_bundle_id,
        from_browser,
        from_profile,
    );

    if !local && !dry_run {
        match try_daemon_route_open(&context) {
            Ok(decision) => {
                print_decision(&decision);
                return Ok(());
            }
            Err(IpcError::Offline) => {
                eprintln!("linkpilot: daemon offline, falling back to local execution");
            }
            Err(IpcError::Other(msg)) => {
                return Err(anyhow!("daemon: {msg}"));
            }
        }
    }

    let (store, _) = load_store(config)?;
    let doc = store.document();
    let decision = Router::new(&doc).evaluate(&context);
    print_decision(&decision);

    if dry_run {
        return Ok(());
    }
    let platform = make_platform();
    match decision {
        RoutingDecision::Open { target, .. } => {
            let parsed = url::Url::parse(&url).with_context(|| format!("parsing URL {url}"))?;
            platform
                .url_launcher()
                .open(&target, &parsed)
                .map_err(|e| anyhow!("launcher: {e}"))?;
            Ok(())
        }
        RoutingDecision::Allow { .. } => {
            eprintln!("linkpilot: 'allow' has no source browser when invoked from CLI");
            Ok(())
        }
        RoutingDecision::Ask { .. } => Err(anyhow!("'ask' is not handled in headless CLI mode")),
        RoutingDecision::Block { reason } => Err(anyhow!("blocked: {reason}")),
    }
}

// ===========================================================================
// `lpt doctor`

fn run_doctor(config: Option<PathBuf>, local: bool, json: bool) -> Result<()> {
    if !local {
        match try_daemon_doctor() {
            Ok(report) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_doctor_report(&report, None);
                }
                return Ok(());
            }
            Err(IpcError::Offline) => {
                eprintln!("linkpilot: daemon offline, running doctor locally");
            }
            Err(IpcError::Other(msg)) => return Err(anyhow!("daemon: {msg}")),
        }
    }
    let (store, _) = load_store(config)?;
    let platform = make_platform();
    let installed = platform
        .browser_inventory()
        .installed_browsers()
        .map_err(|e| anyhow!("inventory: {e}"))?;
    let report = DoctorReport {
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        is_default_browser: platform
            .default_browser()
            .is_linkpilot_default()
            .unwrap_or(false),
        config_path: Some(store.path().display().to_string()),
        installed_browser_count: installed.len(),
        ipc_socket_path: None,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_doctor_report(&report, Some(&*platform));
    }
    Ok(())
}

// ===========================================================================
// `lpt rules ...`

fn run_rules_list(config: Option<PathBuf>, local: bool, json: bool, all: bool) -> Result<()> {
    let doc = read_doc(config, local)?;
    // List order IS priority — walk top-to-bottom, no sort.
    let rules: Vec<_> = doc.rules.iter().filter(|r| all || r.enabled).collect();
    if json {
        for r in &rules {
            println!("{}", serde_json::to_string(r)?);
        }
        return Ok(());
    }
    if rules.is_empty() {
        println!("(no rules)");
        return Ok(());
    }
    let width = (rules.len().max(1).to_string().len()).max(2);
    for (idx, rule) in rules.iter().enumerate() {
        let id8 = short_id(&rule.id.0.to_string());
        let flag = if rule.enabled { ' ' } else { '!' };
        let workspace = rule
            .workspace_id
            .as_deref()
            .map(|w| format!(" ws={w}"))
            .unwrap_or_default();
        println!(
            "{flag} #{:>width$} {} {}{} -> {}",
            idx + 1,
            id8,
            describe_when(&rule.when),
            workspace,
            describe_action(&rule.then),
            width = width,
        );
    }
    Ok(())
}

fn run_rules_show(config: Option<PathBuf>, local: bool, prefix: &str) -> Result<()> {
    let doc = read_doc(config, local)?;
    let rule = find_rule(&doc, prefix)?;
    println!("{}", serde_json::to_string_pretty(rule)?);
    Ok(())
}

fn run_rules_add(config: Option<PathBuf>, args: Box<RuleAddArgs>) -> Result<()> {
    let when = build_matcher(&args)?;
    let then = build_action(&args)?;
    let rule = Rule {
        id: RuleId::new(),
        enabled: !args.disabled,
        when,
        then,
        source: RuleSource::Gui,
        note: args.note,
        workspace_id: args.workspace,
    };
    // Append: new rule lands at the bottom (lowest priority). The
    // user explicitly promotes it with `lpt rules move <id> top|up`
    // — safer default than auto-overriding existing rules.
    mutate_local(config, |doc| {
        doc.rules.push(rule.clone());
        Ok(())
    })?;
    println!("{}", rule.id.0);
    Ok(())
}

fn run_rules_delete(config: Option<PathBuf>, prefix: &str) -> Result<()> {
    mutate_local(config, |doc| {
        let id = find_rule(doc, prefix)?.id.clone();
        doc.rules.retain(|r| r.id != id);
        eprintln!("linkpilot: deleted rule {}", id.0);
        Ok(())
    })
}

fn run_rules_set_enabled(config: Option<PathBuf>, prefix: &str, enabled: bool) -> Result<()> {
    mutate_local(config, |doc| {
        let id = find_rule(doc, prefix)?.id.clone();
        let rule = doc.rules.iter_mut().find(|r| r.id == id).unwrap();
        rule.enabled = enabled;
        eprintln!(
            "linkpilot: {} rule {}",
            if enabled { "enabled" } else { "disabled" },
            id.0
        );
        Ok(())
    })
}

fn run_rules_move(config: Option<PathBuf>, prefix: &str, pos: MovePosition) -> Result<()> {
    mutate_local(config, |doc| {
        let id = find_rule(doc, prefix)?.id.clone();
        let idx = doc.rules.iter().position(|r| r.id == id).unwrap();
        let target = match pos {
            MovePosition::Top => 0,
            MovePosition::Up => idx.saturating_sub(1),
            MovePosition::Down => (idx + 1).min(doc.rules.len() - 1),
            MovePosition::Bottom => doc.rules.len() - 1,
        };
        if target != idx {
            let rule = doc.rules.remove(idx);
            doc.rules.insert(target, rule);
        }
        eprintln!(
            "linkpilot: rule {} moved to position {} (1 = top, {} = bottom)",
            id.0,
            target + 1,
            doc.rules.len()
        );
        Ok(())
    })
}

fn build_matcher(a: &RuleAddArgs) -> Result<MatcherTree> {
    if let Some(json) = &a.when_json {
        return serde_json::from_str(json).context("parsing --when-json");
    }
    let mut clauses: Vec<MatcherTree> = Vec::new();
    if let Some(p) = &a.host {
        clauses.push(MatcherTree::UrlHost { pattern: p.clone() });
    }
    if let Some(p) = &a.path {
        clauses.push(MatcherTree::UrlPath { pattern: p.clone() });
    }
    if let Some(name) = &a.from_app {
        clauses.push(MatcherTree::SourceApp {
            name: name.clone(),
            bundle_id: a.from_app_bundle_id.clone(),
        });
    }
    if let Some(b) = &a.from_browser {
        clauses.push(MatcherTree::SourceBrowser { browser: b.clone() });
    }
    if let Some(p) = &a.from_profile {
        clauses.push(MatcherTree::SourceProfile { profile: p.clone() });
    }
    match clauses.len() {
        0 => Err(anyhow!(
            "rule needs at least one matcher (--host / --path / --from-app / --from-browser / --from-profile / --when-json)"
        )),
        1 => Ok(clauses.into_iter().next().unwrap()),
        _ => Ok(MatcherTree::All { of: clauses }),
    }
}

fn build_action(a: &RuleAddArgs) -> Result<Action> {
    if let Some(json) = &a.then_json {
        return serde_json::from_str(json).context("parsing --then-json");
    }
    if let Some(browser) = &a.target {
        let mut target = BrowserTarget::new(BrowserId::new(browser.clone()));
        if let Some(p) = &a.target_profile {
            target = target.with_profile(p.clone());
        }
        target.incognito = a.incognito;
        target.new_window = a.new_window;
        return Ok(Action::Open { target });
    }
    if a.keep_source {
        return Ok(Action::KeepSource);
    }
    if a.ask {
        return Ok(Action::Ask);
    }
    if a.block {
        return Ok(Action::Block);
    }
    Err(anyhow!(
        "rule needs an action (--target BROWSER / --keep-source / --ask / --block / --then-json)"
    ))
}

fn find_rule<'a>(doc: &'a ConfigDocument, prefix: &str) -> Result<&'a Rule> {
    let matches: Vec<&Rule> = doc
        .rules
        .iter()
        .filter(|r| r.id.0.to_string().starts_with(prefix))
        .collect();
    match matches.as_slice() {
        [] => Err(anyhow!("no rule matches id prefix '{prefix}'")),
        [r] => Ok(r),
        many => Err(anyhow!(
            "id prefix '{prefix}' is ambiguous — matches {} rules",
            many.len()
        )),
    }
}

// ===========================================================================
// `lpt workspaces ...`

fn run_workspaces_list(config: Option<PathBuf>, local: bool, json: bool) -> Result<()> {
    let doc = read_doc(config, local)?;
    if json {
        for w in &doc.workspaces {
            println!("{}", serde_json::to_string(w)?);
        }
        return Ok(());
    }
    if doc.workspaces.is_empty() {
        println!("(no workspaces)");
        return Ok(());
    }
    for w in &doc.workspaces {
        let flag = if w.enabled { ' ' } else { '!' };
        let desc = w
            .description
            .as_deref()
            .map(|d| format!(" — {d}"))
            .unwrap_or_default();
        let count = doc
            .rules
            .iter()
            .filter(|r| r.workspace_id.as_deref() == Some(w.id.as_str()))
            .count();
        println!(
            "{flag} {:24} {:30} {:>3} rule(s){}",
            w.id, w.display_name, count, desc
        );
    }
    Ok(())
}

fn run_workspaces_add(
    config: Option<PathBuf>,
    id: String,
    name: String,
    description: Option<String>,
    enabled: bool,
) -> Result<()> {
    mutate_local(config, |doc| {
        let ws = Workspace {
            id: id.clone(),
            display_name: name.clone(),
            description: description.clone(),
            enabled,
        };
        if let Some(existing) = doc.workspaces.iter_mut().find(|w| w.id == ws.id) {
            *existing = ws;
            eprintln!("linkpilot: updated workspace {id}");
        } else {
            doc.workspaces.push(ws);
            eprintln!("linkpilot: created workspace {id}");
        }
        Ok(())
    })
}

fn run_workspaces_delete(config: Option<PathBuf>, id: &str) -> Result<()> {
    mutate_local(config, |doc| {
        let before = doc.workspaces.len();
        doc.workspaces.retain(|w| w.id != id);
        if doc.workspaces.len() == before {
            return Err(anyhow!("workspace not found: {id}"));
        }
        // Mirror the GUI: rules that pointed at the deleted workspace
        // revert to "ungrouped" instead of dangling.
        for rule in &mut doc.rules {
            if rule.workspace_id.as_deref() == Some(id) {
                rule.workspace_id = None;
            }
        }
        eprintln!("linkpilot: deleted workspace {id}");
        Ok(())
    })
}

fn run_workspaces_set_enabled(config: Option<PathBuf>, id: &str, enabled: bool) -> Result<()> {
    mutate_local(config, |doc| {
        let Some(ws) = doc.workspaces.iter_mut().find(|w| w.id == id) else {
            return Err(anyhow!("workspace not found: {id}"));
        };
        ws.enabled = enabled;
        eprintln!(
            "linkpilot: workspace {id} {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    })
}

// ===========================================================================
// `lpt config ...`

fn run_config_show(config: Option<PathBuf>, local: bool) -> Result<()> {
    let doc = read_doc(config, local)?;
    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

fn run_config_path(config: Option<PathBuf>) -> Result<()> {
    let path = match config {
        Some(p) => p,
        None => default_config_path().context("resolving default config path")?,
    };
    println!("{}", path.display());
    Ok(())
}

fn run_config_import(config: Option<PathBuf>, src: PathBuf) -> Result<()> {
    let raw =
        std::fs::read_to_string(&src).with_context(|| format!("reading {}", src.display()))?;
    let doc: ConfigDocument =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", src.display()))?;
    let (store, _) = load_store(config)?;
    store
        .replace(doc, WriterId::Cli)
        .map_err(|e| anyhow!("writing config: {e}"))?;
    eprintln!("linkpilot: imported config from {}", src.display());
    Ok(())
}

fn run_config_export(config: Option<PathBuf>, local: bool, dest: PathBuf) -> Result<()> {
    let doc = read_doc(config, local)?;
    let json = serde_json::to_string_pretty(&doc)?;
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(&dest, json).with_context(|| format!("writing {}", dest.display()))?;
    eprintln!("linkpilot: exported config to {}", dest.display());
    Ok(())
}

fn run_config_set_default_target(
    config: Option<PathBuf>,
    browser: String,
    profile: Option<String>,
    incognito: bool,
    new_window: bool,
) -> Result<()> {
    mutate_local(config, |doc| {
        let mut target = BrowserTarget::new(BrowserId::new(browser.clone()));
        if let Some(p) = &profile {
            target = target.with_profile(p.clone());
        }
        target.incognito = incognito;
        target.new_window = new_window;
        doc.default_target = target;
        eprintln!(
            "linkpilot: default target set to {browser}{}",
            profile
                .as_deref()
                .map(|p| format!(" / {p}"))
                .unwrap_or_default()
        );
        Ok(())
    })
}

/// `lpt config compile <source.ts> [--to PATH]`.
///
/// Pipeline:
///   1. Locate `bun` on PATH; fail fast with an install hint if missing.
///   2. `bun run <source>` — bun handles .ts natively (~30ms cold).
///   3. Capture stdout, JSON-parse into ConfigDocument. The DSL's
///      `printConfig` helper writes exactly that to stdout.
///   4. Write: either `ConfigStore::replace(doc, WriterId::TsCompiled)`
///      to the live config (default), or `--to PATH` to a file.
///
/// On TS compile error: bun's stderr is passed through verbatim, our
/// own exit is 1, no file is touched.
fn run_config_compile(config: Option<PathBuf>, source: PathBuf, to: Option<PathBuf>) -> Result<()> {
    use std::process::Command;

    if !source.exists() {
        return Err(anyhow!("source file not found: {}", source.display()));
    }

    let bun = which_on_path("bun").ok_or_else(|| {
        anyhow!(
            "`bun` not found on PATH.\n\
             Install with:\n    \
                 brew install oven-sh/bun/bun\n\
             or:\n    \
                 curl -fsSL https://bun.sh/install | bash"
        )
    })?;

    let out = Command::new(&bun)
        .arg("run")
        .arg(&source)
        .output()
        .with_context(|| format!("spawning {}", bun.display()))?;
    if !out.status.success() {
        // Bun's stderr is the user's TS compile error (or runtime
        // error) — surfacing it verbatim is the actionable thing.
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprint!("{stderr}");
        return Err(anyhow!(
            "bun exited {} compiling {}",
            out.status,
            source.display()
        ));
    }

    let stdout = String::from_utf8(out.stdout)
        .with_context(|| format!("bun stdout from {} was not UTF-8", source.display()))?;
    let doc: ConfigDocument = serde_json::from_str(&stdout).with_context(|| {
        format!(
            "DSL output is not a valid ConfigDocument.\n\
             Make sure your config ends with `printConfig(defineConfig({{ ... }}))` \
             — the helper from @linkpilot/config that emits the daemon's JSON shape.\n\
             First 200 bytes of bun stdout: {}",
            stdout.chars().take(200).collect::<String>()
        )
    })?;

    if let Some(dest) = to {
        // --to PATH: write the JSON verbatim, never touch the daemon's
        // live config. Same atomic write semantics as `lpt config export`.
        let json = serde_json::to_string_pretty(&doc)?;
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
        }
        std::fs::write(&dest, json).with_context(|| format!("writing {}", dest.display()))?;
        eprintln!(
            "linkpilot: compiled {} → {} ({} rules)",
            source.display(),
            dest.display(),
            doc.rules.len()
        );
        return Ok(());
    }

    // Default path: replace the live daemon config. Tagged TsCompiled so
    // the GUI knows to render these rules read-only (M4.4).
    let (store, _) = load_store(config)?;
    store
        .replace(doc.clone(), WriterId::TsCompiled)
        .map_err(|e| anyhow!("writing config: {e}"))?;
    eprintln!(
        "linkpilot: compiled {} → {} ({} rules)",
        source.display(),
        store.path().display(),
        doc.rules.len()
    );
    Ok(())
}

/// Look up an executable on PATH. Returns the first match, or None if
/// nothing on PATH has the given filename. Cheap enough to call once
/// per invocation; we keep `daemon_cli::which_on_path` separate to
/// avoid pulling the macOS-gated `mod` from outside its cfg.
fn which_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(bin);
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

// ===========================================================================
// `lpt settings ...`

fn run_settings_show(config: Option<PathBuf>, local: bool) -> Result<()> {
    let doc = read_doc(config, local)?;
    println!("{}", serde_json::to_string_pretty(&doc.settings)?);
    Ok(())
}

fn run_settings_set<F>(config: Option<PathBuf>, mutator: F) -> Result<()>
where
    F: FnOnce(&mut linkpilot_core::config::Settings),
{
    mutate_local(config, |doc| {
        mutator(&mut doc.settings);
        eprintln!(
            "linkpilot: settings = {}",
            serde_json::to_string(&doc.settings).unwrap_or_default()
        );
        Ok(())
    })
}

// ===========================================================================
// `lpt browsers ...`

fn run_browsers_list(
    config: Option<PathBuf>,
    local: bool,
    json: bool,
    installed_only: bool,
) -> Result<()> {
    let platform = make_platform();
    let detected = platform
        .browser_inventory()
        .installed_browsers()
        .unwrap_or_default();
    let custom = if installed_only {
        Vec::new()
    } else {
        read_doc(config, local)?.custom_browsers
    };

    // Mirror the GUI's merge: custom entries override auto-detected ones
    // with the same id (the user explicitly edited that browser).
    let mut merged: Vec<InstalledBrowser> = detected;
    for c in &custom {
        if let Some(slot) = merged.iter_mut().find(|b| b.id == c.id) {
            *slot = c.clone();
        } else {
            merged.push(c.clone());
        }
    }

    if json {
        for b in &merged {
            println!("{}", serde_json::to_string(b)?);
        }
        return Ok(());
    }
    if merged.is_empty() {
        println!("(no browsers detected)");
        return Ok(());
    }
    let custom_ids: std::collections::HashSet<&BrowserId> = custom.iter().map(|c| &c.id).collect();
    for b in &merged {
        let marker = if custom_ids.contains(&b.id) { '+' } else { ' ' };
        println!(
            "{marker} {:14} {:24} {:?} {}",
            b.id.to_string(),
            b.display_name,
            b.kind,
            b.executable.display()
        );
    }
    Ok(())
}

fn run_browsers_profiles(browser: &str, json: bool) -> Result<()> {
    let platform = make_platform();
    let id = BrowserId::new(browser);
    let profiles: Vec<BrowserProfile> = platform
        .browser_inventory()
        .profiles(&id)
        .map_err(|e| anyhow!("inventory: {e}"))?;
    if json {
        for p in &profiles {
            println!("{}", serde_json::to_string(p)?);
        }
        return Ok(());
    }
    if profiles.is_empty() {
        println!("(no profiles)");
        return Ok(());
    }
    for p in &profiles {
        let email = p
            .email
            .as_deref()
            .map(|e| format!("  <{e}>"))
            .unwrap_or_default();
        println!("  {:24} {}{}", p.id, p.display_name, email);
    }
    Ok(())
}

fn run_browsers_custom_add(
    config: Option<PathBuf>,
    id: String,
    name: String,
    kind: BrowserKind,
    exec: PathBuf,
    bundle_id: Option<String>,
) -> Result<()> {
    let browser = InstalledBrowser {
        id: BrowserId::new(id.clone()),
        display_name: name,
        kind,
        executable: exec,
        platform_app_id: bundle_id,
        profile_root: None,
    };
    mutate_local(config, |doc| {
        if let Some(slot) = doc.custom_browsers.iter_mut().find(|b| b.id == browser.id) {
            *slot = browser.clone();
            eprintln!("linkpilot: updated custom browser {id}");
        } else {
            doc.custom_browsers.push(browser.clone());
            eprintln!("linkpilot: added custom browser {id}");
        }
        Ok(())
    })
}

fn run_browsers_custom_remove(config: Option<PathBuf>, id: &str) -> Result<()> {
    mutate_local(config, |doc| {
        let id_obj = BrowserId::new(id);
        let before = doc.custom_browsers.len();
        doc.custom_browsers.retain(|b| b.id != id_obj);
        if doc.custom_browsers.len() == before {
            return Err(anyhow!("custom browser not found: {id}"));
        }
        eprintln!("linkpilot: removed custom browser {id}");
        Ok(())
    })
}

// ===========================================================================
// `lpt default-browser ...`

fn run_default_browser_status(json: bool) -> Result<()> {
    let platform = make_platform();
    let current = platform.default_browser().current_default().ok().flatten();
    let is_us = platform
        .default_browser()
        .is_linkpilot_default()
        .unwrap_or(false);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "is_linkpilot_default": is_us,
                "current_default": current.as_ref().map(|c| c.0.clone()),
            }))?
        );
    } else {
        println!("is_linkpilot_default: {is_us}");
        println!(
            "current_default:      {}",
            current
                .as_ref()
                .map(|c| c.0.as_str())
                .unwrap_or("(unknown)")
        );
    }
    Ok(())
}

fn run_default_browser_set() -> Result<()> {
    let platform = make_platform();
    match platform
        .default_browser()
        .request_set_default()
        .map_err(|e| anyhow!("default-browser: {e}"))?
    {
        SetDefaultOutcome::Done => {
            eprintln!("linkpilot: registered as default browser");
        }
        SetDefaultOutcome::UserConsentRequired { instructions_url } => {
            eprintln!("linkpilot: user consent required");
            if let Some(url) = instructions_url {
                eprintln!("  see {url}");
            }
        }
        SetDefaultOutcome::NotSupported => {
            return Err(anyhow!(
                "setting default browser is not supported on this platform"
            ));
        }
    }
    Ok(())
}

// ===========================================================================
// Daemon IPC helpers

fn new_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("lpt-{nanos}")
}

enum IpcError {
    Offline,
    Other(String),
}

fn classify_ipc_error(err: ClientError) -> IpcError {
    match err {
        ClientError::Offline(_) => IpcError::Offline,
        ClientError::Timeout(_) => IpcError::Offline,
        other => IpcError::Other(other.to_string()),
    }
}

fn try_daemon_route_open(
    context: &RoutingContext,
) -> std::result::Result<RoutingDecision, IpcError> {
    let request = Request::RouteOpen {
        request_id: new_request_id(),
        context: context.clone(),
    };
    let response = client::send(&default_endpoint(), request).map_err(classify_ipc_error)?;
    match response {
        Response::RouteDecision { decision, .. } => Ok(decision),
        Response::Error { code, message, .. } => Err(IpcError::Other(format!("{code}: {message}"))),
        other => Err(IpcError::Other(format!("unexpected response: {other:?}"))),
    }
}

fn try_daemon_doctor() -> std::result::Result<DoctorReport, IpcError> {
    let request = Request::Doctor {
        request_id: new_request_id(),
    };
    let response = client::send(&default_endpoint(), request).map_err(classify_ipc_error)?;
    match response {
        Response::DoctorReport { report, .. } => Ok(report),
        Response::Error { code, message, .. } => Err(IpcError::Other(format!("{code}: {message}"))),
        other => Err(IpcError::Other(format!("unexpected response: {other:?}"))),
    }
}

fn try_daemon_config_get() -> std::result::Result<ConfigDocument, IpcError> {
    let request = Request::ConfigGet {
        request_id: new_request_id(),
    };
    let response = client::send(&default_endpoint(), request).map_err(classify_ipc_error)?;
    match response {
        Response::ConfigSnapshot { doc, .. } => Ok(doc),
        Response::Error { code, message, .. } => Err(IpcError::Other(format!("{code}: {message}"))),
        other => Err(IpcError::Other(format!("unexpected response: {other:?}"))),
    }
}

/// Distinguishes "daemon doesn't speak this verb" from generic IPC
/// errors. `lpt history` uses this to print an upgrade hint instead of
/// a low-level "unknown-verb" code dump when talking to an old daemon.
enum HistoryError {
    Offline,
    UnknownVerb,
    Other(String),
}

fn try_daemon_route_history(limit: usize) -> std::result::Result<Vec<RouteRecord>, HistoryError> {
    let request = Request::RouteHistory {
        request_id: new_request_id(),
        limit: Some(limit),
    };
    let response = client::send(&default_endpoint(), request).map_err(|e| match e {
        ClientError::Offline(_) | ClientError::Timeout(_) => HistoryError::Offline,
        // A v0.1 daemon (or any pre-M3.2 v0.2 daemon) drops the
        // connection on a route-history frame — its serde decode fails
        // and the old server path returned Err(...). From the client
        // side that surfaces as Transport(Closed) or Transport(Serde).
        // Both mean "daemon accepted the request but couldn't speak v2".
        ClientError::Transport(TransportError::Closed)
        | ClientError::Transport(TransportError::Serde(_)) => HistoryError::UnknownVerb,
        other => HistoryError::Other(other.to_string()),
    })?;
    match response {
        Response::RouteHistorySnapshot { records, .. } => Ok(records),
        Response::Error { code, message, .. } if code == ERROR_UNKNOWN_VERB => {
            // v0.2-post-M3.2 daemons send this when they don't know the
            // verb. Same human-facing hint as the connection-closed
            // case above.
            tracing::debug!(code = %code, message = %message, "daemon rejected route-history");
            Err(HistoryError::UnknownVerb)
        }
        Response::Error { code, message, .. } => {
            Err(HistoryError::Other(format!("{code}: {message}")))
        }
        other => Err(HistoryError::Other(format!(
            "unexpected response: {other:?}"
        ))),
    }
}

// ===========================================================================
// Local config store helpers

fn load_store(override_path: Option<PathBuf>) -> Result<(ConfigStore, bool)> {
    let path = match override_path {
        Some(p) => p,
        None => default_config_path().context("resolving default config path")?,
    };
    let (store, created) = ConfigStore::load_or_init(path.clone())
        .with_context(|| format!("loading config from {}", path.display()))?;
    if created {
        eprintln!(
            "linkpilot: initialised default config at {}",
            path.display()
        );
    }
    Ok((store, created))
}

/// Read the live document, daemon-first.
fn read_doc(config: Option<PathBuf>, local: bool) -> Result<ConfigDocument> {
    if !local {
        match try_daemon_config_get() {
            Ok(d) => return Ok(d),
            Err(IpcError::Offline) => {}
            Err(IpcError::Other(msg)) => return Err(anyhow!("daemon: {msg}")),
        }
    }
    Ok(load_store(config)?.0.document())
}

/// Mutate the on-disk config atomically. A running daemon's fsnotify watcher
/// picks the change up via the anti-echo token so the GUI refreshes.
fn mutate_local<F>(config: Option<PathBuf>, mutator: F) -> Result<()>
where
    F: FnOnce(&mut ConfigDocument) -> Result<()>,
{
    let (store, _) = load_store(config)?;
    let mut doc = store.document();
    mutator(&mut doc)?;
    store
        .replace(doc, WriterId::Cli)
        .map_err(|e| anyhow!("writing config: {e}"))?;
    Ok(())
}

// ===========================================================================
// Formatters

fn build_context(
    url: &str,
    from_app: Option<String>,
    from_app_bundle_id: Option<String>,
    from_browser: Option<String>,
    from_profile: Option<String>,
) -> RoutingContext {
    // Source.kind matches what the Tauri GUI's Test-URL panel sends: if a
    // source browser is named the event is browser-extension-shaped, else
    // it's a CLI handoff.
    let kind = if from_browser.is_some() {
        SourceKind::BrowserExtension
    } else {
        SourceKind::Cli
    };
    RoutingContext {
        url: url.to_string(),
        source: Source {
            kind,
            app_name: from_app,
            bundle_id: from_app_bundle_id,
            browser: from_browser,
            profile: from_profile,
        },
        navigation: None,
        environment: None,
    }
}

fn print_decision(decision: &RoutingDecision) {
    match decision {
        RoutingDecision::Open {
            target,
            matched_rule,
            reason,
        } => {
            let profile = target
                .profile
                .as_deref()
                .map(|p| format!(" / profile={p}"))
                .unwrap_or_default();
            let rule = matched_rule
                .as_ref()
                .map(|r| format!(" rule={}", short_id(&r.0.to_string())))
                .unwrap_or_else(|| " (default target)".to_string());
            eprintln!(
                "linkpilot: open → {}{profile}{rule} :: {reason}",
                target.browser
            );
        }
        other => eprintln!("linkpilot: {other:?}"),
    }
}

fn print_doctor_report(report: &DoctorReport, platform: Option<&dyn PlatformProvider>) {
    println!("LinkPilot doctor");
    println!("  daemon version:        {}", report.daemon_version);
    println!("  is default browser:    {}", report.is_default_browser);
    println!(
        "  config path:           {}",
        report.config_path.as_deref().unwrap_or("-")
    );
    println!(
        "  installed browsers:    {}",
        report.installed_browser_count
    );
    println!(
        "  ipc socket:            {}",
        report.ipc_socket_path.as_deref().unwrap_or("-")
    );

    if let Some(platform) = platform {
        if let Ok(browsers) = platform.browser_inventory().installed_browsers() {
            for b in browsers {
                let profiles = platform
                    .browser_inventory()
                    .profiles(&b.id)
                    .unwrap_or_default();
                println!(
                    "    {:10} {:24} {}",
                    b.id.to_string(),
                    b.display_name,
                    b.executable.display()
                );
                for p in profiles {
                    println!("        profile: {:24} ({})", p.display_name, p.id);
                }
            }
        }
    }
}

fn describe_when(tree: &MatcherTree) -> String {
    match tree {
        MatcherTree::Always => "always".into(),
        MatcherTree::UrlHost { pattern } => format!("host {pattern}"),
        MatcherTree::UrlPath { pattern } => format!("path {pattern}"),
        MatcherTree::SourceApp { name, .. } => format!("from app {name}"),
        MatcherTree::SourceBrowser { browser } => format!("from browser {browser}"),
        MatcherTree::SourceProfile { profile } => format!("from profile {profile}"),
        MatcherTree::All { of } => of
            .iter()
            .map(describe_when)
            .collect::<Vec<_>>()
            .join(" AND "),
        MatcherTree::Any { of } => of
            .iter()
            .map(describe_when)
            .collect::<Vec<_>>()
            .join(" OR "),
        MatcherTree::Not { of } => format!("NOT ({})", describe_when(of)),
    }
}

fn describe_action(action: &Action) -> String {
    match action {
        Action::Open { target } => {
            let profile = target
                .profile
                .as_deref()
                .map(|p| format!("/{p}"))
                .unwrap_or_default();
            let flags = match (target.incognito, target.new_window) {
                (true, true) => " (incognito,new-window)",
                (true, false) => " (incognito)",
                (false, true) => " (new-window)",
                (false, false) => "",
            };
            format!("open → {}{profile}{flags}", target.browser)
        }
        Action::KeepSource => "keep-source".into(),
        Action::Ask => "ask".into(),
        Action::Block => "block".into(),
    }
}

fn short_id(uuid: &str) -> String {
    uuid.chars().take(8).collect()
}

// ===========================================================================
// `lpt daemon <action>` (M2)
//
// All daemon-management subcommands are macOS-only in v0.2. The Linux /
// Windows daemon ports land later; in the meantime non-macOS hosts get a
// single friendly error rather than a half-working install path.

#[cfg(not(target_os = "macos"))]
fn run_daemon_unsupported(action: &str) -> Result<()> {
    Err(anyhow!(
        "`lpt daemon {action}` is macOS-only in v0.2 (no daemon shipped on this platform yet)"
    ))
}

#[cfg(not(target_os = "macos"))]
fn run_daemon_start() -> Result<()> {
    run_daemon_unsupported("start")
}
#[cfg(not(target_os = "macos"))]
fn run_daemon_stop() -> Result<()> {
    run_daemon_unsupported("stop")
}
#[cfg(not(target_os = "macos"))]
fn run_daemon_restart() -> Result<()> {
    run_daemon_unsupported("restart")
}
#[cfg(not(target_os = "macos"))]
fn run_daemon_status(_json: bool) -> Result<()> {
    run_daemon_unsupported("status")
}
#[cfg(not(target_os = "macos"))]
fn run_daemon_install() -> Result<()> {
    run_daemon_unsupported("install")
}
#[cfg(not(target_os = "macos"))]
fn run_daemon_uninstall() -> Result<()> {
    run_daemon_unsupported("uninstall")
}
#[cfg(not(target_os = "macos"))]
fn run_daemon_logs(_follow: bool, _lines: usize) -> Result<()> {
    run_daemon_unsupported("logs")
}

#[cfg(target_os = "macos")]
mod daemon_cli {
    use anyhow::{anyhow, Context, Result};
    use linkpilot_core::daemon::{pid_file_path, read_pid_file, remove_pid_file};
    use linkpilot_core::protocol::{Request, Response};
    use linkpilot_ipc::client;
    use linkpilot_ipc::path::default_endpoint;
    use linkpilot_platform_mac::launch_agent::{self, LaunchAgentStatus, DAEMON_LABEL};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// Snapshot of every signal `lpt daemon status` cares about, gathered once
    /// per invocation so JSON / human output stay consistent.
    pub struct DaemonSnapshot {
        pub running: bool,
        pub version: Option<String>,
        pub pid: Option<u32>,
        pub socket: String,
        pub agent: LaunchAgentStatus,
        pub pid_file: PathBuf,
    }

    pub fn snapshot() -> Result<DaemonSnapshot> {
        let endpoint = default_endpoint();
        let socket = endpoint.display();
        let ping = client::send(
            &endpoint,
            Request::StatePing {
                request_id: "lpt-daemon-status".into(),
            },
        );
        let (running, version) = match ping {
            Ok(Response::Pong { daemon_version, .. }) => (true, Some(daemon_version)),
            _ => (false, None),
        };
        let pid_file = pid_file_path().context("resolve pid file path")?;
        let pid = read_pid_file(&pid_file).ok().flatten();
        let agent = launch_agent::daemon_status().unwrap_or_default();
        Ok(DaemonSnapshot {
            running,
            version,
            pid,
            socket,
            agent,
            pid_file,
        })
    }

    /// Locate a `linkpilot-daemon` binary the CLI can spawn:
    ///   1. `LINKPILOT_DAEMON` env override (for development).
    ///   2. The installed .app at `/Applications/LinkPilot.app/Contents/MacOS/`.
    ///   3. Sibling of the current `lpt` executable (CI-built `target/...`).
    ///   4. `PATH` lookup of `linkpilot-daemon`.
    pub fn locate_daemon_binary() -> Result<PathBuf> {
        if let Some(p) = std::env::var_os("LINKPILOT_DAEMON") {
            let path = PathBuf::from(p);
            if path.exists() {
                return Ok(path);
            }
        }
        let installed =
            PathBuf::from("/Applications/LinkPilot.app/Contents/MacOS/linkpilot-daemon");
        if installed.exists() {
            return Ok(installed);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let sibling = dir.join("linkpilot-daemon");
                if sibling.exists() {
                    return Ok(sibling);
                }
            }
        }
        if let Ok(path) = which_on_path("linkpilot-daemon") {
            return Ok(path);
        }
        Err(anyhow!(
            "no linkpilot-daemon binary found.\n\
             Looked at: $LINKPILOT_DAEMON, /Applications/LinkPilot.app/Contents/MacOS/, \
             alongside `lpt`, and $PATH.\n\
             Install LinkPilot.app, or `cargo build -p linkpilot-headless-daemon` for dev."
        ))
    }

    fn which_on_path(bin: &str) -> Result<PathBuf> {
        let path = std::env::var_os("PATH").ok_or_else(|| anyhow!("PATH not set"))?;
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(bin);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        Err(anyhow!("{bin} not on PATH"))
    }

    /// Spawn `linkpilot-daemon --serve` detached from this process. Stdout
    /// and stderr go to /dev/null — if the user wants logs they install
    /// the LaunchAgent (which writes to ~/Library/Logs/LinkPilot/).
    pub fn spawn_daemon(exec: &Path) -> Result<()> {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        let mut cmd = Command::new(exec);
        cmd.arg("--serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Detach into its own session so a `lpt daemon start` from a
        // terminal that later exits doesn't drag the daemon with it.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let _child = cmd
            .spawn()
            .with_context(|| format!("spawning {}", exec.display()))?;
        Ok(())
    }

    /// Poll the IPC socket until either (a) it stops answering — when we
    /// want to confirm a stop — or (b) it starts answering — when we
    /// want to confirm a start. Returns `Ok(())` on success, error after
    /// `timeout`.
    pub fn wait_for_socket(target_alive: bool, timeout: Duration) -> Result<()> {
        let endpoint = default_endpoint();
        let started = Instant::now();
        while started.elapsed() < timeout {
            let alive = matches!(
                client::send(
                    &endpoint,
                    Request::StatePing {
                        request_id: "lpt-daemon-wait".into()
                    }
                ),
                Ok(Response::Pong { .. })
            );
            if alive == target_alive {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err(anyhow!(
            "timed out after {:?} waiting for socket to {}",
            timeout,
            if target_alive { "open" } else { "close" }
        ))
    }

    /// SIGTERM the PID written by the running daemon. Returns Ok if the
    /// PID file is missing (already stopped) or if the kill succeeded.
    pub fn terminate_daemon(pid: u32) -> Result<()> {
        // libc::pid_t is i32; PIDs fit.
        let res = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if res == 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        // ESRCH = process already gone. That's fine for our intent.
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        Err(anyhow!("SIGTERM to pid {pid} failed: {err}"))
    }

    pub fn log_paths() -> Result<(PathBuf, PathBuf)> {
        let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME not set"))?;
        let dir = PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join("LinkPilot");
        Ok((dir.join("daemon.out.log"), dir.join("daemon.err.log")))
    }

    pub fn pretty_status(snap: &DaemonSnapshot) {
        println!(
            "daemon:       {}",
            if snap.running { "running" } else { "stopped" }
        );
        if let Some(v) = &snap.version {
            println!("version:      {v}");
        }
        match snap.pid {
            Some(p) => println!("pid:          {p} (from {})", snap.pid_file.display()),
            None => println!("pid:          —"),
        }
        println!("socket:       {}", snap.socket);
        println!(
            "launchagent:  plist {} | loaded {} | label {DAEMON_LABEL}",
            if snap.agent.plist_exists { "yes" } else { "no" },
            if snap.agent.loaded { "yes" } else { "no" }
        );
        if let Some(exec) = &snap.agent.exec_path {
            println!("              exec {}", exec.display());
        }
        if let Some(p) = snap.agent.pid {
            println!("              launchd pid {p}");
        }
    }

    pub fn json_status(snap: &DaemonSnapshot) -> serde_json::Value {
        serde_json::json!({
            "running": snap.running,
            "version": snap.version,
            "pid": snap.pid,
            "socket": snap.socket,
            "pid_file": snap.pid_file.display().to_string(),
            "launch_agent": {
                "plist_exists": snap.agent.plist_exists,
                "loaded": snap.agent.loaded,
                "pid": snap.agent.pid,
                "exec_path": snap.agent.exec_path.as_ref().map(|p| p.display().to_string()),
                "label": DAEMON_LABEL,
            }
        })
    }

    /// Stream `path` to stdout, optionally following the tail. Falls back
    /// to a friendly message rather than hard-failing on missing log files
    /// (a freshly-installed daemon hasn't written anything yet).
    pub fn tail_log(path: &Path, lines: usize, follow: bool) -> Result<()> {
        use std::io::{Read, Seek, SeekFrom};
        if !path.exists() {
            println!(
                "{}: no log file yet (daemon hasn't started under launchd, or just installed)",
                path.display()
            );
            return Ok(());
        }
        let body =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let kept: Vec<&str> = body.lines().rev().take(lines).collect();
        for line in kept.iter().rev() {
            println!("{line}");
        }
        if !follow {
            return Ok(());
        }
        // Naive follow: poll size every 250ms; print new chunk.
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("opening {} for follow", path.display()))?;
        file.seek(SeekFrom::End(0))?;
        loop {
            let mut buf = Vec::new();
            let n = file
                .by_ref()
                .take(64 * 1024)
                .read_to_end(&mut buf)
                .unwrap_or(0);
            if n > 0 {
                print!("{}", String::from_utf8_lossy(&buf));
                use std::io::Write;
                let _ = std::io::stdout().flush();
            } else {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }

    /// Remove the PID file. Logged at the call site since callers want to
    /// know whether the daemon was already orphaned vs. cleanly stopped.
    pub fn clear_pid_file() {
        if let Ok(p) = pid_file_path() {
            let _ = remove_pid_file(&p);
        }
    }
}

#[cfg(target_os = "macos")]
fn run_daemon_start() -> Result<()> {
    use daemon_cli::*;
    let snap = snapshot()?;
    if snap.running {
        println!("daemon already running (pid {})", snap.pid.unwrap_or(0));
        return Ok(());
    }
    let exec = locate_daemon_binary()?;
    spawn_daemon(&exec)?;
    wait_for_socket(true, std::time::Duration::from_secs(5))
        .context("daemon spawned but socket never came up; check ~/Library/Logs/LinkPilot/")?;
    let after = snapshot()?;
    println!(
        "daemon started (pid {}, version {})",
        after.pid.unwrap_or(0),
        after.version.as_deref().unwrap_or("?")
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_daemon_stop() -> Result<()> {
    use daemon_cli::*;
    let snap = snapshot()?;
    if snap.agent.loaded {
        return Err(anyhow!(
            "daemon is managed by launchd — run `lpt daemon uninstall` first \
             (otherwise launchd would re-spawn it immediately)"
        ));
    }
    let Some(pid) = snap.pid else {
        if snap.running {
            return Err(anyhow!(
                "daemon socket responds but no PID file at {} — can't safely SIGTERM. \
                 Find the process manually with `pgrep -fl linkpilot-daemon` and `kill` it.",
                snap.pid_file.display()
            ));
        }
        println!("daemon not running.");
        clear_pid_file();
        return Ok(());
    };
    terminate_daemon(pid)?;
    wait_for_socket(false, std::time::Duration::from_secs(5))
        .context("SIGTERM sent but daemon didn't release the socket")?;
    clear_pid_file();
    println!("daemon stopped (was pid {pid})");
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_daemon_restart() -> Result<()> {
    run_daemon_stop()?;
    run_daemon_start()
}

#[cfg(target_os = "macos")]
fn run_daemon_status(json: bool) -> Result<()> {
    let snap = daemon_cli::snapshot()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&daemon_cli::json_status(&snap))?
        );
    } else {
        daemon_cli::pretty_status(&snap);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_daemon_install() -> Result<()> {
    use daemon_cli::*;
    let exec = locate_daemon_binary()?;
    linkpilot_platform_mac::launch_agent::install_daemon(&exec)
        .map_err(|e| anyhow!("install LaunchAgent: {e}"))?;
    // launchctl load -w with RunAtLoad=true triggers an immediate start —
    // give it a moment to come up so `lpt daemon status` right after this
    // shows running=true.
    let _ = wait_for_socket(true, std::time::Duration::from_secs(5));
    let snap = snapshot()?;
    println!(
        "daemon LaunchAgent installed (exec {}; running: {})",
        exec.display(),
        snap.running
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_daemon_uninstall() -> Result<()> {
    use daemon_cli::*;
    let snap = snapshot()?;
    linkpilot_platform_mac::launch_agent::uninstall_daemon()
        .map_err(|e| anyhow!("uninstall LaunchAgent: {e}"))?;
    // launchctl unload normally SIGTERMs the daemon. Belt-and-suspenders:
    // if a PID file still exists and the process is still alive, signal it.
    if let Some(pid) = snap.pid {
        let _ = terminate_daemon(pid);
    }
    let _ = wait_for_socket(false, std::time::Duration::from_secs(3));
    clear_pid_file();
    println!("daemon LaunchAgent uninstalled");
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_daemon_logs(follow: bool, lines: usize) -> Result<()> {
    let (out, err) = daemon_cli::log_paths()?;
    println!("=== {} ===", out.display());
    daemon_cli::tail_log(&out, lines, false)?;
    println!();
    println!("=== {} ===", err.display());
    daemon_cli::tail_log(&err, lines, follow)?;
    Ok(())
}

// ===========================================================================
// `lpt history` (M3)

fn run_history(limit: usize, json: bool) -> Result<()> {
    let records = match try_daemon_route_history(limit) {
        Ok(records) => records,
        Err(HistoryError::Offline) => {
            return Err(anyhow!(
                "history requires a running daemon — try `lpt daemon start`"
            ));
        }
        Err(HistoryError::UnknownVerb) => {
            return Err(anyhow!(
                "the running daemon doesn't speak protocol v2; upgrade it to v0.2+ to use `lpt history`"
            ));
        }
        Err(HistoryError::Other(msg)) => return Err(anyhow!("daemon: {msg}")),
    };

    if records.is_empty() {
        println!("no routes recorded yet");
        return Ok(());
    }

    if json {
        for rec in &records {
            // One RouteRecord per line so `lpt history --json | jq` works
            // line-at-a-time, same shape the daemon stores in memory.
            println!("{}", serde_json::to_string(rec)?);
        }
        return Ok(());
    }

    // Human table — matches the column ordering of `lpt rules list` so a
    // user moving between the two sees the same shape.
    println!("{:<10}  {:<20}  {:<8}  decision", "time", "host", "rule");
    for rec in &records {
        let when = format_relative(rec.timestamp_ms);
        let host = url_host(&rec.context.url);
        let rule = rec
            .matched_rule
            .as_ref()
            .map(|r| short_id(&r.0.to_string()))
            .unwrap_or_else(|| "—".into());
        let decision = format_decision(&rec.decision);
        println!(
            "{:<10}  {:<20}  {:<8}  {}",
            when,
            truncate(&host, 20),
            rule,
            decision,
        );
    }
    Ok(())
}

fn url_host(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => u.host_str().unwrap_or(url).to_string(),
        Err(_) => url.to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let kept: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

fn format_relative(ts_ms: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(ts_ms);
    let delta = now_ms.saturating_sub(ts_ms) / 1000;
    if delta < 60 {
        format!("{delta}s ago")
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86_400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86_400)
    }
}

fn format_decision(d: &RoutingDecision) -> String {
    match d {
        RoutingDecision::Open { target, .. } => {
            let profile = target
                .profile
                .as_deref()
                .map(|p| format!("/{p}"))
                .unwrap_or_default();
            format!("open → {}{profile}", target.browser)
        }
        RoutingDecision::Allow { .. } => "allow".into(),
        RoutingDecision::Block { .. } => "block".into(),
        RoutingDecision::Ask { .. } => "ask".into(),
    }
}
