//! `lp` — the LinkPilot command-line client.
//!
//! v0.1 step 5 wires `lp open` and `lp doctor` to operate **directly** against
//! the on-disk config and the platform crate, without going through an IPC
//! server. This gives an end-to-end demo (Slack-style smoke test) while the
//! daemon's socket layer is still under construction.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use linkpilot_core::config::{default_config_path, ConfigStore};
use linkpilot_core::platform::PlatformProvider;
use linkpilot_core::routing::{Router, RoutingContext, RoutingDecision, Source, SourceKind};

#[derive(Parser, Debug)]
#[command(name = "lp", version, about = "LinkPilot command-line client")]
struct Cli {
    /// Override the config file path (defaults to the platform-specific location).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

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
    Box::new(linkpilot_platform_mac::MacProvider::new())
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
    let config_path = match cli.config {
        Some(p) => p,
        None => default_config_path().context("resolving default config path")?,
    };
    let (store, created) = ConfigStore::load_or_init(config_path.clone())
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    if created {
        eprintln!(
            "linkpilot: initialised default config at {}",
            config_path.display()
        );
    }

    let platform = make_platform();

    match cli.command {
        Command::Open {
            url,
            from_app,
            dry_run,
        } => cmd_open(&store, &*platform, &url, from_app, dry_run),
        Command::Doctor => cmd_doctor(&store, &*platform),
        Command::Rules {
            action: RulesAction::List,
        } => cmd_rules_list(&store),
    }
}

fn cmd_open(
    store: &ConfigStore,
    platform: &dyn PlatformProvider,
    url: &str,
    from_app: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let parsed = url::Url::parse(url).with_context(|| format!("parsing URL {url}"))?;
    let context = RoutingContext {
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
    };

    let router = Router::new(store.document());
    let decision = router.evaluate(&context);
    print_decision(&decision);

    if dry_run {
        return Ok(());
    }

    match decision {
        RoutingDecision::Open { target, .. } => {
            platform
                .url_launcher()
                .open(&target, &parsed)
                .map_err(|e| anyhow!("launcher: {e}"))?;
            Ok(())
        }
        RoutingDecision::Allow { .. } => {
            eprintln!("linkpilot: 'allow' has no source browser when invoked from CLI; nothing to do");
            Ok(())
        }
        RoutingDecision::Ask { .. } => Err(anyhow!("'ask' is not handled in headless CLI mode yet")),
        RoutingDecision::Block { reason } => Err(anyhow!("blocked: {reason}")),
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

fn cmd_doctor(store: &ConfigStore, platform: &dyn PlatformProvider) -> Result<()> {
    println!("LinkPilot doctor");
    println!("  config:   {}", store.path().display());
    println!("  rules:    {}", store.document().rules.len());
    println!(
        "  default:  {} / profile={}",
        store.document().default_target.browser,
        store
            .document()
            .default_target
            .profile
            .as_deref()
            .unwrap_or("-")
    );

    let installed = platform
        .browser_inventory()
        .installed_browsers()
        .map_err(|e| anyhow!("inventory: {e}"))?;
    println!("  installed browsers ({}):", installed.len());
    for b in &installed {
        let profiles = platform
            .browser_inventory()
            .profiles(&b.id)
            .map_err(|e| anyhow!("profiles({}): {e}", b.id))?;
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

    let is_default = platform
        .default_browser()
        .is_linkpilot_default()
        .unwrap_or(false);
    println!("  is LinkPilot default browser: {is_default}");
    Ok(())
}

fn cmd_rules_list(store: &ConfigStore) -> Result<()> {
    let mut rules: Vec<_> = store.document().rules.iter().collect();
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

fn describe_when(tree: &linkpilot_core::rules::MatcherTree) -> String {
    use linkpilot_core::rules::MatcherTree;
    match tree {
        MatcherTree::Always => "always".into(),
        MatcherTree::UrlHost { pattern } => format!("host {pattern}"),
        MatcherTree::UrlPath { pattern } => format!("path {pattern}"),
        MatcherTree::SourceApp { name } => format!("from app {name}"),
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
