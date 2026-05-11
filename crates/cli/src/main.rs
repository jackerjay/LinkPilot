//! `lp` — the LinkPilot CLI.
//!
//! v0.1 step 1 stubs out the command surface so callers and shell completions
//! can already target a stable interface. Real IPC dispatch lands once the
//! daemon's socket server exists (step 4).

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "lp", version, about = "LinkPilot command-line client")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Open a URL through the LinkPilot router.
    Open {
        url: String,
    },
    /// Diagnose default-browser, config, and IPC socket state.
    Doctor,
    /// Inspect or modify rules.
    Rules {
        #[command(subcommand)]
        action: RulesAction,
    },
}

#[derive(Subcommand, Debug)]
enum RulesAction {
    List,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    let endpoint = linkpilot_ipc::path::default_endpoint();

    match cli.command {
        Command::Open { url } => {
            println!("lp open {url}");
            println!("(stub) would dispatch RouteOpen via {endpoint:?}");
        }
        Command::Doctor => {
            println!("LinkPilot doctor (stub)");
            println!("  endpoint: {endpoint:?}");
            println!("  daemon:   not yet implemented");
        }
        Command::Rules { action } => match action {
            RulesAction::List => {
                println!("(stub) would fetch ConfigSnapshot and print rules");
            }
        },
    }
    Ok(())
}
