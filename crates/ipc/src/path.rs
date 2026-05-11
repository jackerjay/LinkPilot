//! Default IPC endpoint per platform.

use std::path::PathBuf;

/// Where the daemon listens by default.
///
/// - macOS:   `$HOME/Library/Application Support/LinkPilot/linkpilot.sock`
/// - Linux:   `$XDG_RUNTIME_DIR/linkpilot.sock` (falls back to `$HOME`)
/// - Windows: `\\.\pipe\linkpilot`
pub fn default_endpoint() -> Endpoint {
    #[cfg(target_os = "macos")]
    {
        let base = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        Endpoint::UnixSocket(
            base.join("Library")
                .join("Application Support")
                .join("LinkPilot")
                .join("linkpilot.sock"),
        )
    }
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        Endpoint::UnixSocket(base.join("linkpilot.sock"))
    }
    #[cfg(target_os = "windows")]
    {
        Endpoint::NamedPipe(r"\\.\pipe\linkpilot".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Endpoint::UnixSocket(PathBuf::from("/tmp/linkpilot.sock"))
    }
}

#[derive(Debug, Clone)]
pub enum Endpoint {
    UnixSocket(PathBuf),
    NamedPipe(String),
}
