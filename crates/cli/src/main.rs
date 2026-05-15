//! `lp` — the LinkPilot command-line client.
//!
//! v0.1: prefer the running daemon (via IPC); fall back to direct local
//! execution if no daemon is listening. Both paths share the same router and
//! platform code, so behaviour matches end-to-end.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use linkpilot_core::config::{default_config_path, ConfigStore};
use linkpilot_core::platform::PlatformProvider;
use linkpilot_core::protocol::{DoctorReport, Request, Response};
use linkpilot_core::routing::{Router, RoutingContext, RoutingDecision, Source, SourceKind};
use linkpilot_ipc::client::{self, ClientError};
use linkpilot_ipc::path::default_endpoint;

#[derive(Parser, Debug)]
#[command(name = "lp", version, about = "LinkPilot command-line client")]
struct Cli {
    /// Override the config file path (defaults to the platform location).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Skip the daemon and always execute locally.
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
        /// Show the decision without launching anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Diagnose default-browser, config, and installed-browser state.
    Doctor,
    /// Inspect or modify rules.
    Rules {
        #[command(subcommand)]
        action: RulesAction,
    },
}

#[derive(Subcommand, Debug)]
enum RulesAction {
    /// Print every rule in the config in priority order.
    List,
}

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
            dry_run,
        } => run_open(cli.config, cli.local, url, from_app, dry_run),
        Command::Doctor => run_doctor(cli.config, cli.local),
        Command::Rules {
            action: RulesAction::List,
        } => run_rules_list(cli.config, cli.local),
    }
}

fn new_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("lp-{nanos}")
}

// ---------------------------------------------------------------------------
// `lp open`

fn run_open(
    config: Option<PathBuf>,
    local: bool,
    url: String,
    from_app: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let context = build_context(&url, from_app);

    // Daemon path: route + launch happen in-daemon, ensuring it's the same
    // process the menu bar / GUI sees.
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

    // Local path: load config, evaluate, launch via the platform crate.
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

// ---------------------------------------------------------------------------
// `lp doctor`

fn run_doctor(config: Option<PathBuf>, local: bool) -> Result<()> {
    if !local {
        match try_daemon_doctor() {
            Ok(report) => {
                print_doctor_report(&report, None);
                return Ok(());
            }
            Err(IpcError::Offline) => {
                eprintln!("linkpilot: daemon offline, running doctor locally");
            }
            Err(IpcError::Other(msg)) => {
                return Err(anyhow!("daemon: {msg}"));
            }
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
    print_doctor_report(&report, Some(&*platform));
    Ok(())
}

// ---------------------------------------------------------------------------
// `lp rules list`

fn run_rules_list(config: Option<PathBuf>, local: bool) -> Result<()> {
    let doc = if !local {
        match try_daemon_config_get() {
            Ok(d) => Some(d),
            Err(IpcError::Offline) => None,
            Err(IpcError::Other(msg)) => return Err(anyhow!("daemon: {msg}")),
        }
    } else {
        None
    };
    let doc = match doc {
        Some(d) => d,
        None => load_store(config)?.0.document(),
    };

    let mut rules: Vec<_> = doc.rules.iter().collect();
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    if rules.is_empty() {
        println!("(no rules)");
        return Ok(());
    }
    for rule in rules {
        println!(
            "[{:>4}] {} -> {:?}",
            rule.priority,
            describe_when(&rule.when),
            rule.then
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers

fn build_context(url: &str, from_app: Option<String>) -> RoutingContext {
    RoutingContext {
        url: url.to_string(),
        source: Source {
            kind: SourceKind::Cli,
            app_name: from_app,
            bundle_id: None,
            browser: None,
            profile: None,
        },
        navigation: None,
        environment: None,
    }
}

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

fn try_daemon_config_get() -> std::result::Result<linkpilot_core::config::ConfigDocument, IpcError>
{
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
                .map(|r| format!(" rule={}", r.0))
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

fn describe_when(tree: &linkpilot_core::rules::MatcherTree) -> String {
    use linkpilot_core::rules::MatcherTree;
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
