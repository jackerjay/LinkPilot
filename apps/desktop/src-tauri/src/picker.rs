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
    // apply_glass + elevate_above_fullscreen both reach into AppKit
    // (NSVisualEffectView, NSWindow.setCollectionBehavior /
    // setLevel:). Those MUST run on the main thread or the process
    // crashes. show_picker itself is called from a worker thread
    // (see dispatch.rs spawn), so dispatch back.
    let win_for_main = win.clone();
    let _ = app.run_on_main_thread(move || {
        apply_glass(&win_for_main);
        elevate_above_fullscreen(&win_for_main);
        let _ = win_for_main.set_focus();
    });
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
/// background. Sidebar material gives a pronounced "Cmd-Tab"
/// frosted look (HudWindow we tried first reads as a flat tint).
/// Falls back silently on non-macOS builds.
fn apply_glass(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        match apply_vibrancy(
            window,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::Active),
            Some(16.0), // matches the inner content's rounded-2xl
        ) {
            Ok(()) => tracing::debug!("picker: vibrancy applied"),
            Err(err) => tracing::warn!(?err, "picker: vibrancy failed"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }
}

/// Make the picker float over a full-screen Space (Lark, Slack,
/// browser fullscreen, etc.) and sit above all normal floating
/// windows.
///
/// Two layers:
///   1. Tauri's safe `set_visible_on_all_workspaces(true)` sets the
///      CanJoinAllSpaces bit. By itself this still doesn't appear on
///      full-screen Spaces.
///   2. We additionally toggle the FullScreenAuxiliary bit via raw
///      msg_send! to NSWindow.setCollectionBehavior:, plus raise the
///      window level to NSStatusWindowLevel (25) — above the level
///      Slack / Lark's own floats use.
fn elevate_above_fullscreen(window: &tauri::WebviewWindow) {
    if let Err(err) = window.set_visible_on_all_workspaces(true) {
        tracing::warn!(?err, "picker: set_visible_on_all_workspaces failed");
    }

    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;

        // NSWindowCollectionBehaviorCanJoinAllSpaces    = 1 << 0   = 1
        // NSWindowCollectionBehaviorFullScreenAuxiliary = 1 << 8   = 256
        // NSUInteger on 64-bit macOS == usize; using usize is what
        // objc2 wants for the message arg type.
        const COLLECTION_BEHAVIOR: usize = 1 | (1 << 8);
        // NSStatusWindowLevel
        const NS_STATUS_WINDOW_LEVEL: isize = 25;

        let raw = match window.ns_window() {
            Ok(p) => p as *mut AnyObject,
            Err(err) => {
                tracing::warn!(?err, "picker: ns_window unavailable");
                return;
            }
        };
        if raw.is_null() {
            tracing::warn!("picker: ns_window null");
            return;
        }
        unsafe {
            let ns: &AnyObject = &*raw;
            let _: () = msg_send![ns, setCollectionBehavior: COLLECTION_BEHAVIOR];
            let _: () = msg_send![ns, setLevel: NS_STATUS_WINDOW_LEVEL];
        }
        tracing::debug!("picker: collection behavior + level applied");
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
