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
use tauri::{ActivationPolicy, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// One option in the chooser. The renderer uses `name` for display.
///
/// `icon_data_url` is pre-rendered on the Rust side before the picker
/// window opens — every `show_picker` call spins up a fresh webview
/// (the window is closed on dismissal), which means the renderer's
/// in-memory icon cache is empty on each open. Without pre-rendering
/// the user sees a brief blank-icon → real-icon flash on every Ask;
/// embedding the base64 PNG in the session payload makes the picker
/// fully painted on first frame.
#[derive(Debug, Clone, Serialize)]
pub struct PickerChoice {
    pub id: String,
    pub name: String,
    pub bundle_id: Option<String>,
    pub app_path: Option<String>,
    #[serde(default)]
    pub icon_data_url: Option<String>,
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
    mut choices: Vec<PickerChoice>,
) -> Option<String> {
    // Pre-render every choice's icon as a base64 data URL so the
    // picker webview can paint the icon row on its very first frame
    // — see `PickerChoice::icon_data_url` for why this matters.
    for c in choices.iter_mut() {
        if c.icon_data_url.is_some() {
            continue;
        }
        c.icon_data_url = render_icon_data_url(
            c.bundle_id.as_deref(),
            c.app_path.as_deref(),
            Some(c.name.as_str()),
        );
    }

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

    // Critical for full-screen Space overlay: temporarily flip the
    // NSApplication activation policy to Accessory. In Regular mode
    // (the default — LinkPilot has a Dock icon and a main window
    // that lives in some "home" Space), activating the app to show
    // the picker triggers a Space-switch back to that home Space.
    // Accessory apps have no home Space; their windows materialize
    // in whatever Space is currently active (i.e. the user's
    // fullscreen Lark / Safari Space), which combined with the
    // FullScreenAuxiliary collection bit is exactly what Spotlight,
    // Raycast, and Alfred do. Restored to Regular below so the Dock
    // icon comes back as soon as the user picks or cancels.
    let _ = app.set_activation_policy(ActivationPolicy::Accessory);

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
        // Build invisible so the vibrancy + collection-behavior bits
        // land BEFORE first orderFront. Otherwise the window briefly
        // belongs to LinkPilot's home Space before we tag it as
        // fullscreen-auxiliary, and macOS pins it there.
        .visible(false)
        .focused(true);
    match center_position_on_cursor_monitor(app, 560.0, 280.0) {
        Some((x, y)) => builder = builder.position(x, y),
        None => builder = builder.center(),
    }
    let win = match builder.build() {
        Ok(w) => w,
        Err(err) => {
            tracing::warn!(?err, "picker: window build failed");
            // Don't leave the app stuck without a Dock icon when the
            // picker never even materialised.
            let _ = app.set_activation_policy(ActivationPolicy::Regular);
            return None;
        }
    };
    // apply_glass + elevate_above_fullscreen both reach into AppKit
    // (NSVisualEffectView, NSWindow.setCollectionBehavior /
    // setLevel:). Those MUST run on the main thread or the process
    // crashes. show_picker itself is called from a worker thread
    // (see dispatch.rs spawn), so dispatch back.
    let win_for_main = win.clone();
    let _ = app.run_on_main_thread(move || {
        apply_glass(&win_for_main);
        elevate_above_fullscreen(&win_for_main);
        // Now that the AppKit bits are in place, show + focus. Order
        // matters: show first (orderFrontRegardless under the hood),
        // THEN set_focus so the webview is key window for keystrokes.
        let _ = win_for_main.show();
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

    // Restore the Dock icon + hand off foreground. The "how" depends
    // on whether the user picked a browser or cancelled:
    //
    // PICKED (result.is_some()):
    //   Call NSApp.hide:nil — equivalent of ⌘H. Hides ALL of
    //   LinkPilot's windows AND deactivates in one atomic AppKit
    //   call. Why this and not plain deactivate: deactivate drops
    //   foreground but leaves visible windows on screen; AppKit's
    //   "this app still has visible content" heuristic then
    //   re-promotes us in the next event-loop tick, beating the
    //   browser's own NSApp.activate. Result the user saw: Chrome
    //   opens the URL, focus snaps back to LinkPilot's config
    //   window. Minimised LinkPilot didn't show the bug because
    //   minimised windows don't trigger the heuristic. Trade-off:
    //   main window goes away after every Ask resolve — Spotlight /
    //   Raycast model; the Dock icon + tray icon are both still
    //   present, one click brings the main window back.
    //
    // CANCELLED (result.is_none()):
    //   Plain NSApp.deactivate. Nothing's competing for foreground
    //   so the lighter-weight call is enough — and we don't
    //   surprise the user by vanishing their main window when they
    //   pressed Esc to cancel.
    let app_for_main = app.clone();
    let picked = result.is_some();
    let _ = app.run_on_main_thread(move || {
        let _ = app_for_main.set_activation_policy(ActivationPolicy::Regular);
        #[cfg(target_os = "macos")]
        unsafe {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;
            let cls = objc2::class!(NSApplication);
            let ns_app: *mut AnyObject = msg_send![cls, sharedApplication];
            if !ns_app.is_null() {
                let nil: *mut AnyObject = std::ptr::null_mut();
                if picked {
                    let _: () = msg_send![&*ns_app, hide: nil];
                } else {
                    let _: () = msg_send![&*ns_app, deactivate];
                }
            }
        }
    });

    tracing::debug!(?result, "picker: resolved");
    result
}

#[tauri::command]
pub fn picker_session(state: tauri::State<'_, PickerState>) -> Option<PickerSession> {
    state.session.lock().ok().and_then(|g| g.clone())
}

/// Best-effort `.icns → 64pt PNG → base64 data URL` rendering for the
/// picker pre-paint. Mirrors `commands::app_icon` but inlined so we
/// don't have to round-trip through the renderer. Returns `None` when
/// the platform doesn't support icon extraction, the bundle isn't
/// found, or the cached PNG is unreadable — the renderer falls back
/// to a generic lucide glyph in that case.
fn render_icon_data_url(
    bundle_id: Option<&str>,
    app_path: Option<&str>,
    name: Option<&str>,
) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use base64::Engine;
        let bundle = bundle_id.filter(|s| !s.is_empty());
        let path = app_path.filter(|s| !s.is_empty()).map(std::path::Path::new);
        let name = name.filter(|s| !s.is_empty());
        let png_path =
            match linkpilot_platform_mac::app_icon::ensure_png(bundle, path, name, 64) {
                Ok(p) => p,
                Err(err) => {
                    tracing::debug!(?err, ?bundle, ?path, ?name, "picker: icon prefetch failed");
                    return None;
                }
            };
        let bytes = std::fs::read(&png_path).ok()?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Some(format!("data:image/png;base64,{b64}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (bundle_id, app_path, name);
        None
    }
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
/// background. HudWindow is what Spotlight / Raycast / Cmd-Tab use —
/// it desaturates less than Sidebar so colors from the desktop bleed
/// through (the "Control Center" feel the user asked for).
fn apply_glass(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        match apply_vibrancy(
            window,
            NSVisualEffectMaterial::HudWindow,
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
        // NSWindowCollectionBehaviorStationary          = 1 << 4   = 16
        //   (Stationary keeps the window from animating into Mission
        //    Control / Exposé thumbnails when shown over fullscreen.)
        // NSUInteger on 64-bit macOS == usize; using usize is what
        // objc2 wants for the message arg type.
        const COLLECTION_BEHAVIOR: usize = 1 | (1 << 8) | (1 << 4);
        // NSPopUpMenuWindowLevel = 101. Sits above the fullscreen
        // app's chrome layer (which can hide windows at the lower
        // NSStatusWindowLevel = 25); same level Raycast uses for its
        // global hotkey window.
        const NS_POPUP_MENU_WINDOW_LEVEL: isize = 101;

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
            let _: () = msg_send![ns, setLevel: NS_POPUP_MENU_WINDOW_LEVEL];
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
