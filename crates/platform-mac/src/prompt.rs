//! Native browser-choice prompt for routes whose action is `ask`.
//!
//! Uses AppleScript's `choose from list` — the same chooser macOS itself
//! uses when you "Open With…" a file with no default. Single Tk-style
//! sheet, returns the picked label or None on cancel. We translate that
//! back to a BrowserId in the caller.
//!
//! Why osascript rather than a Tauri window: every URL event fires when
//! LinkPilot is not necessarily in the foreground (Slack → click link
//! → daemon catches the AE). Building a custom window per ask would
//! mean steering focus, sizing, theming, and ESC handling ourselves.
//! `choose from list` gives us all of that for free.

use std::process::Command;

const CANCEL_SENTINEL: &str = "__cancelled__";

/// Show the chooser. Returns `Some(label)` of the picked entry or
/// `None` if the user dismissed the dialog.
pub fn pick_browser(url: &str, choices: &[String]) -> Option<String> {
    if choices.is_empty() {
        return None;
    }

    // Build the AppleScript list literal: {"Chrome", "Arc", "Safari"}.
    // Escape any embedded quotes by AppleScript's rules (double the ").
    let list_items = choices
        .iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(", ");

    let prompt = format!("Open {url} in:");
    let script = format!(
        r#"
set theList to {{{list_items}}}
set theChoice to choose from list theList with prompt "{prompt}" with title "LinkPilot" default items {{item 1 of theList}}
if theChoice is false then
    return "{CANCEL_SENTINEL}"
else
    return (item 1 of theChoice) as text
end if
"#,
        prompt = applescript_quote(&prompt),
    );

    let output = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if result.is_empty() || result == CANCEL_SENTINEL {
        return None;
    }
    Some(result)
}

/// Minimal AppleScript-string escaping for content we drop inside `"…"`.
/// AppleScript escapes embedded quotes by doubling them.
fn applescript_quote(s: &str) -> String {
    s.replace('"', "\"\"")
}
