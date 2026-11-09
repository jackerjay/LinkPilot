use clap::{Parser, Subcommand};
#[cfg(target_os = "macos")]
mod service;

#[derive(Parser)]
#[command(
    version,
    about = "Optional browser-profile companion for LinkPilot Store"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Subcommand)]
enum Action {
    /// Stop and unregister the per-user Helper service.
    Disable,
    /// Internal launchd entrypoint.
    #[command(hide = true)]
    Serve,
    /// Check the running Helper's authenticated protocol and capabilities.
    Status,
    /// Enumerate profiles through the same authenticated channel as Store.
    Profiles { browser: String },
    /// Deliver a web link to an existing browser profile.
    Open {
        #[arg(long)]
        browser: String,
        #[arg(long)]
        profile: String,
        url: String,
    },
}

#[cfg(target_os = "macos")]
fn run() -> Result<(), String> {
    use linkpilot_core::browser::{BrowserId, BrowserTarget};
    use linkpilot_helper_ipc::{transport, Command, Response};
    let args = Args::parse();
    if matches!(args.command, Some(Action::Serve)) {
        return transport::serve(linkpilot_platform_mac::helper::handle);
    }
    if matches!(args.command, Some(Action::Disable)) {
        return service::uninstall();
    }
    let command = match args.command {
        Some(Action::Serve | Action::Disable) => unreachable!(),
        Some(Action::Status) => Some(Command::Status),
        Some(Action::Profiles { browser }) => Some(Command::Profiles {
            browser: BrowserId::new(browser),
        }),
        Some(Action::Open {
            browser,
            profile,
            url,
        }) => Some(Command::Open {
            target: BrowserTarget::new(BrowserId::new(browser)).with_profile(profile),
            url,
        }),
        None => None,
    };
    if let Some(command) = command {
        let response = transport::request(command)?;
        if let Response::Error { message } = response {
            return Err(message);
        }
        println!(
            "{}",
            serde_json::to_string(&response).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    service::install()?;
    extern "C" fn disable() -> i32 {
        match service::uninstall() {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        }
    }
    extern "C" {
        fn lp_helper_menu(disable: extern "C" fn() -> i32);
    }
    unsafe {
        lp_helper_menu(disable);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run() -> Result<(), String> {
    Err("LinkPilot Helper requires macOS".into())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
