//! `lp` — the LinkPilot command-line client.
//!
//! Read operations prefer the running daemon (via IPC) so they reflect the
//! daemon's in-memory snapshot; on offline daemon they fall back to reading
//! the on-disk config + platform inventory directly. Write operations always
//! mutate the on-disk config file atomically — the daemon's fsnotify watcher
//! (with its anti-echo token) picks them up and reloads, so a running GUI
//! refreshes within a frame.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use linkpilot_core::browser::{
    BrowserId, BrowserKind, BrowserProfile, BrowserTarget, InstalledBrowser,
};
use linkpilot_core::config::{
    default_config_path, ConfigDocument, ConfigStore, Workspace, WriterId,
};
use linkpilot_core::platform::{PlatformProvider, SetDefaultOutcome};
use linkpilot_core::protocol::{DoctorReport, Request, Response};
use linkpilot_core::routing::{Router, RoutingContext, RoutingDecision, Source, SourceKind};
use linkpilot_core::rules::{Action, MatcherTree, Rule, RuleId, RuleSource};
use linkpilot_ipc::client::{self, ClientError};
use linkpilot_ipc::path::default_endpoint;

// ---------------------------------------------------------------------------
// Clap structures

#[derive(Parser, Debug)]
#[command(name = "lp", version, about = "LinkPilot command-line client")]
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
}

// --- rules ---

#[derive(Subcommand, Debug)]
enum RulesAction {
    /// Print every rule in the config in priority order.
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
    /// Create a new rule.
    Add(Box<RuleAddArgs>),
    /// Delete a rule by id (full or prefix; prefix must be unambiguous).
    Delete { id: String },
    /// Mark a rule enabled.
    Enable { id: String },
    /// Mark a rule disabled.
    Disable { id: String },
    /// Set a rule's priority. Higher wins; ties broken by list order.
    SetPriority { id: String, priority: i32 },
}

#[derive(clap::Args, Debug)]
struct RuleAddArgs {
    /// Higher priority wins. Defaults to 10 — same as the demo rules.
    #[arg(long, default_value_t = 10)]
    priority: i32,
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
    /// Toggle whether URL query strings are kept in route history.
    RecordQueryStrings { value: OnOff },
    /// Set history retention in days, or `clear` to keep forever.
    HistoryRetention { value: String },
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
            RulesAction::SetPriority { id, priority } => {
                run_rules_set_priority(cli.config, &id, priority)
            }
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
        },
        Command::Settings { action } => match action {
            SettingsAction::Show => run_settings_show(cli.config, cli.local),
            SettingsAction::SmartRouting { value } => {
                run_settings_set(cli.config, |s| s.smart_routing_enabled = value.as_bool())
            }
            SettingsAction::LaunchAtLogin { value } => {
                run_settings_set(cli.config, |s| s.launch_at_login = value.as_bool())
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
    }
}

// ===========================================================================
// `lp open`

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
// `lp doctor`

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
// `lp rules ...`

fn run_rules_list(config: Option<PathBuf>, local: bool, json: bool, all: bool) -> Result<()> {
    let doc = read_doc(config, local)?;
    let mut rules: Vec<_> = doc.rules.iter().filter(|r| all || r.enabled).collect();
    rules.sort_by_key(|r| std::cmp::Reverse(r.priority));
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
    for rule in rules {
        let id8 = short_id(&rule.id.0.to_string());
        let flag = if rule.enabled { ' ' } else { '!' };
        let workspace = rule
            .workspace_id
            .as_deref()
            .map(|w| format!(" ws={w}"))
            .unwrap_or_default();
        println!(
            "{flag} {} [{:>4}] {}{} -> {}",
            id8,
            rule.priority,
            describe_when(&rule.when),
            workspace,
            describe_action(&rule.then),
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
        priority: args.priority,
        enabled: !args.disabled,
        when,
        then,
        source: RuleSource::Gui,
        note: args.note,
        workspace_id: args.workspace,
    };
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

fn run_rules_set_priority(config: Option<PathBuf>, prefix: &str, priority: i32) -> Result<()> {
    mutate_local(config, |doc| {
        let id = find_rule(doc, prefix)?.id.clone();
        let rule = doc.rules.iter_mut().find(|r| r.id == id).unwrap();
        rule.priority = priority;
        eprintln!("linkpilot: rule {} priority set to {priority}", id.0);
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
// `lp workspaces ...`

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
// `lp config ...`

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

// ===========================================================================
// `lp settings ...`

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
// `lp browsers ...`

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
// `lp default-browser ...`

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
    format!("lp-{nanos}")
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
