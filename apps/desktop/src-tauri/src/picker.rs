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

    // Center on the monitor the cursor is on, not always the primary
    // display. Falls back to .center() if we can't resolve a monitor.
    let target = WebviewUrl::App("index.html?view=picker".into());
    let mut builder = WebviewWindowBuilder::new(app, WINDOW_LABEL, target)
        .title("LinkPilot")
        .inner_size(560.0, 280.0)
        .min_inner_size(560.0, 280.0)
        .max_inner_size(560.0, 280.0)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .skip_taskbar(true)
        .focused(true);
    match center_position_on_cursor_monitor(app, 560.0, 280.0) {
        Some((x, y)) => builder = builder.position(x, y),
        None => builder = builder.center(),
    }
    let win = builder.build().ok()?;
    let _ = win.set_focus();
    apply_glass(&win);
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

/// Resolve the centered top-left position (in logical pixels) for a
/// `width x height` window on whichever monitor currently contains the
/// mouse cursor. Returns None when the cursor / monitor lookup fails;
/// caller should fall back to Tauri's `.center()`.
fn center_position_on_cursor_monitor(
    app: &AppHandle,
    width: f64,
    height: f64,
) -> Option<(f64, f64)> {
    let cursor = app.cursor_position().ok()?; // physical pixels
    let monitors = app.available_monitors().ok()?;
    let monitor = monitors.into_iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        let cx = cursor.x;
        let cy = cursor.y;
        cx >= pos.x as f64
            && cx < (pos.x as f64 + size.width as f64)
            && cy >= pos.y as f64
            && cy < (pos.y as f64 + size.height as f64)
    })?;
    let scale = monitor.scale_factor();
    // Monitor.position()/size() are physical; the builder expects
    // logical pixels. Convert by dividing through scale_factor.
    let mx = monitor.position().x as f64 / scale;
    let my = monitor.position().y as f64 / scale;
    let mw = monitor.size().width as f64 / scale;
    let mh = monitor.size().height as f64 / scale;
    Some((mx + (mw - width) / 2.0, my + (mh - height) / 2.0))
}

/// Apply macOS frosted-glass vibrancy to the picker window's
/// background. Falls back silently on non-macOS builds.
fn apply_glass(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        if let Err(err) = apply_vibrancy(
            window,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::Active),
            Some(16.0), // matches the inner content's rounded-2xl
        ) {
            tracing::warn!(?err, "picker: vibrancy failed");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }
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
