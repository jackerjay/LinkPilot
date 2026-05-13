//! Browser-pick UI for routes whose action is `ask`.
//!
//! Replaces the osascript "choose from list" dialog with a custom
//! Tauri secondary window styled like macOS's Cmd-Tab switcher.
//! Lifecycle:
//!   1. dispatch::resolve_ask calls `show_picker(app, url, choices)`.
//!   2. We stash the choices + a oneshot channel sender in PickerState
//!      and open the "picker" window.
//!   3. main.tsx routes to <PickerWindow/> when window.label == "picker".
//!      It calls `picker_session()` for the data, renders, and on
//!      click / Enter / Esc calls `picker_resolve(picked)`.
//!   4. `picker_resolve` drops the picked id through the channel and
//!      `show_picker` returns it.

use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// One option in the chooser. The renderer uses `name` for display +
/// looks up an icon via AppIcon's existing (bundleId | appPath | name)
/// resolution.
#[derive(Debug, Clone, Serialize)]
pub struct PickerChoice {
    pub id: String,
    pub name: String,
    pub bundle_id: Option<String>,
    pub app_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PickerSession {
    pub url: String,
    pub choices: Vec<PickerChoice>,
}

pub struct PickerState {
    pub session: Mutex<Option<PickerSession>>,
    pub pending: Mutex<Option<mpsc::Sender<Option<String>>>>,
}

impl Default for PickerState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
            pending: Mutex::new(None),
        }
    }
}

const WINDOW_LABEL: &str = "picker";
const WINDOW_TIMEOUT_SECS: u64 = 60;

/// Show the picker, block until the user picks or cancels (or 60s
/// elapses), and return the picked choice id.
pub fn show_picker(
    app: &AppHandle,
    url: &str,
    choices: Vec<PickerChoice>,
) -> Option<String> {
    let state: tauri::State<PickerState> = app.state();
    // sync mpsc — we're already on a worker thread, blocking is fine
    // and dodges the tokio dependency.
    let (tx, rx) = mpsc::channel::<Option<String>>();
    {
        if let Ok(mut s) = state.session.lock() {
            *s = Some(PickerSession {
                url: url.to_string(),
                choices,
            });
        }
        if let Ok(mut p) = state.pending.lock() {
            *p = Some(tx);
        }
    }

    // Close any leftover picker window (previous ask crashed mid-flow).
    if let Some(existing) = app.get_webview_window(WINDOW_LABEL) {
        let _ = existing.close();
    }

    let target = WebviewUrl::App("index.html?view=picker".into());
    let win = WebviewWindowBuilder::new(app, WINDOW_LABEL, target)
        .title("LinkPilot")
        .inner_size(560.0, 280.0)
        .min_inner_size(560.0, 280.0)
        .max_inner_size(560.0, 280.0)
        .center()
        .always_on_top(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .skip_taskbar(true)
        .focused(true)
        .build()
        .ok()?;
    let _ = win.set_focus();
    tracing::debug!(%url, "picker: window opened");

    // Sync blocking wait — we're a worker thread, this doesn't tie up
    // the Tauri main event loop.
    let result = match rx.recv_timeout(Duration::from_secs(WINDOW_TIMEOUT_SECS)) {
        Ok(v) => v,
        Err(_) => None,
    };

    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        let _ = win.close();
    }
    if let Ok(mut s) = state.session.lock() {
        *s = None;
    }
    if let Ok(mut p) = state.pending.lock() {
        *p = None;
    }
    tracing::debug!(?result, "picker: resolved");
    result
}

#[tauri::command]
pub fn picker_session(state: tauri::State<'_, PickerState>) -> Option<PickerSession> {
    state.session.lock().ok().and_then(|g| g.clone())
}

#[tauri::command]
pub fn picker_resolve(state: tauri::State<'_, PickerState>, picked: Option<String>) {
    let sender_opt: Option<mpsc::Sender<Option<String>>> = state
        .pending
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    if let Some(tx) = sender_opt {
        let _ = tx.send(picked);
    }
}
