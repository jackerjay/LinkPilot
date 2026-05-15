//! Native Messaging Host: relays length-prefixed JSON between a browser
//! extension (over stdio) and the LinkPilot daemon (over Unix socket /
//! Named pipe).
//!
//! v0.1 step 1 is a placeholder. The real bridge ships with v0.3 alongside
//! the Chromium extension.

use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let endpoint = linkpilot_ipc::path::default_endpoint();
    tracing::info!(
        ?endpoint,
        "linkpilot-native-host placeholder; bridge lands in v0.3"
    );
    Ok(())
}
