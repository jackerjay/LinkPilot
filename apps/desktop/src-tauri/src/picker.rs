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

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use linkpilot_core::config::PickerStyle;
use serde::{Deserialize, Serialize};
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
    /// Profiles available inside this browser, ordered with the
    /// `is_default` entry first so the Halo wheel can index by 1–9 in
    /// a meaningful order. Empty for single-profile browsers (Safari,
    /// fresh Arc installs) — the renderer treats `len() <= 1` as
    /// "no wheel, plain click → launch default".
    #[serde(default)]
    pub profiles: Vec<PickerProfile>,
    /// Convenience pointer to the profile id (within `profiles`) that
    /// the picker should launch when the user clicks the tile without
    /// holding the summon key. `None` falls back to "first entry".
    #[serde(default)]
    pub default_profile_id: Option<String>,
}

/// Slim subset of `BrowserProfile` shipped to the picker renderer.
/// Kept narrow on purpose — the picker only paints a colored avatar +
/// name + email; everything else (avatar PNG, gaia hints) stays
/// daemon-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickerProfile {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    /// Hex `#RRGGBB`. Deterministic per-profile (see
    /// `core::inventory::accent_for`). The wheel uses it for outer-rim
    /// bands, hover wedges, and the Crown center-display avatar.
    pub accent_color: Option<String>,
    /// Mirrors `BrowserProfile::is_default`. Picker renders the
    /// Crown idle preview around the entry where this is true.
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PickerSession {
    pub url: String,
    pub choices: Vec<PickerChoice>,
    /// Halo visual style at the moment the picker opened. The
    /// renderer reads this once and locks the variant for this
    /// session — flipping `picker_style` mid-pick wouldn't have a
    /// sane outcome (Crown's center-display geometry is quite
    /// different from Bezel's).
    pub style: PickerStyle,
}

/// Renderer-side resolution payload. Either a (browser, profile)
/// composite (Halo + Frosted/Bezel/Crown all use this), or just a
/// browser id when the user clicked a single-profile tile without
/// arming the wheel.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PickerPick {
    pub browser_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
}

pub struct PickerState {
    pub session: Mutex<Option<PickerSession>>,
    pub pending: Mutex<Option<mpsc::Sender<Option<PickerPick>>>>,
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
const DEFAULT_PREVIEW_URL: &str = "https://example.com/";

/// Show the picker, block until the user picks or cancels (or 60s
/// elapses), and return the picked (browser, profile) pair.
pub fn show_picker(
    app: &AppHandle,
    url: &str,
    mut choices: Vec<PickerChoice>,
    style: PickerStyle,
) -> Option<PickerPick> {
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
    let (tx, rx) = mpsc::channel::<Option<PickerPick>>();
    // Atomic claim of the picker slot. Ask is a high-frequency action
    // (multiple URLs arriving back-to-back, or a Test URL simulator
    // racing with a real deep-link), and we previously overwrote the
    // pending sender silently — the *first* caller's mpsc::recv would
    // then block until the 60s timeout, and the picked target would
    // route to the wrong URL.
    //
    // We take both locks at once so a competing show_picker call on
    // another thread can't slip between them. On contention we return
    // None — dispatch treats that as "user cancelled / no target", which
    // matches macOS's own behavior when something steals key focus.
    {
        let mut pending = match state.pending.lock() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("picker: pending lock poisoned, refusing");
                return None;
            }
        };
        if pending.is_some() {
            tracing::warn!("picker: another ask is in flight, dropping this one");
            return None;
        }
        let mut session = match state.session.lock() {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!("picker: session lock poisoned, refusing");
                return None;
            }
        };
        *session = Some(PickerSession {
            url: url.to_string(),
            choices,
            style,
        });
        *pending = Some(tx);
    }

    // Close any leftover picker window (previous ask crashed mid-flow).
    if let Some(existing) = app.get_webview_window(WINDOW_LABEL) {
        let _ = existing.close();
    }

    // Belt-and-suspenders: LinkPilot runs permanently in Accessory
    // mode since M5.5 (LSUIElement=true + setup-time set_activation_
    // policy), so this is a no-op for normal startups. Kept as a
    // safety net for unusual launch paths (e.g. tauri dev before the
    // setup() callback runs). Accessory apps have no home Space:
    // their windows materialize in whatever Space is currently active
    // (the user's fullscreen Lark / Safari Space), which combined
    // with the FullScreenAuxiliary collection bit is exactly what
    // Spotlight, Raycast, and Alfred do.
    let _ = app.set_activation_policy(ActivationPolicy::Accessory);

    // Center on the monitor the cursor is on, not always the primary
    // display. Falls back to .center() if we can't resolve a monitor.
    // The Halo wheel (outer radius ~152, plus the floating readout 200px
    // above center) needs significant breathing room around the popover.
    // 720×520 fits any of the three variants — Frosted/Bezel/Crown — with
    // the wheel summoned, and stays compact enough that the rest of the
    // screen still reads as background. The window is transparent +
    // chrome-less, so the only "visible" surface is the central popover;
    // the extra area is the wheel's portaled paint zone.
    let target = WebviewUrl::App("index.html?view=picker".into());
    const WIN_W: f64 = 720.0;
    const WIN_H: f64 = 520.0;
    let mut builder = WebviewWindowBuilder::new(app, WINDOW_LABEL, target)
        .title("LinkPilot")
        .inner_size(WIN_W, WIN_H)
        .min_inner_size(WIN_W, WIN_H)
        .max_inner_size(WIN_W, WIN_H)
        .transparent(true)
        // macOS draws a 1px hairline + drop shadow on every window by
        // default. With apply_glass we used to cover that with a 16px
        // rounded NSVisualEffectView, so the shadow traced the rounded
        // popover shape. Without the vibrancy, the shadow + hairline
        // outline the FULL 720x520 rectangle — visible as a faint dark
        // border around the picker. Disabling the system shadow makes
        // the only visible chrome the popover's own CSS box-shadow.
        .shadow(false)
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
    match center_position_on_cursor_monitor(app, WIN_W, WIN_H) {
        Some((x, y)) => builder = builder.position(x, y),
        None => builder = builder.center(),
    }
    let win = match builder.build() {
        Ok(w) => w,
        Err(err) => {
            tracing::warn!(?err, "picker: window build failed");
            // App stays in Accessory mode (M5.5) — no Dock icon to
            // restore. Just bail.
            return None;
        }
    };
    // We deliberately do NOT apply NSVisualEffectView vibrancy here.
    // The old Cmd-Tab-style picker did, because its 560×280 window
    // was *exactly* the popover; the vibrancy WAS the popover background.
    //
    // The Halo picker is different: the popover is small (~420×~260) and
    // the surrounding window is large (720×520) because the wheel
    // portals out beyond the popover frame. Painting HudWindow vibrancy
    // across the full window leaves an oversized frosted rectangle around
    // a popover that's already doing its own `backdrop-filter: blur(36px)` —
    // the result is two visibly-stacked frosts and a giant "card" that
    // dwarfs the actual UI.
    //
    // Instead, leave the window fully transparent (transparent: true on
    // the builder + body/html/root → transparent in PickerWindow.tsx).
    // The popover paints one frost layer; the Halo wheel's
    // `halo-frost-disc` paints its own when summoned. Everything else
    // shows the desktop through — Spotlight / Raycast style.
    //
    // elevate_above_fullscreen still has to run on the main thread
    // (AppKit msg_send! to NSWindow.setCollectionBehavior).
    let win_for_main = win.clone();
    let _ = app.run_on_main_thread(move || {
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

    // Hand off foreground to the browser that's about to open the URL.
    // We stay in Accessory mode — no Dock icon to restore, no main
    // menu to put back. Behaviour depends on whether the user picked
    // a browser or cancelled:
    //
    // PICKED (result.is_some()):
    //   Call NSApp.hide:nil — equivalent of ⌘H. Hides ALL of
    //   LinkPilot's windows AND deactivates in one atomic AppKit
    //   call. Why this and not plain deactivate: deactivate drops
    //   foreground but leaves visible windows on screen; AppKit's
    //   "this app still has visible content" heuristic can then
    //   re-promote us in the next event-loop tick, beating the
    //   browser's own NSApp.activate. (Less of an issue in Accessory
    //   mode since M5.5, but we keep the explicit hide to be safe.)
    //
    // CANCELLED (result.is_none()):
    //   Plain NSApp.deactivate. Nothing's competing for foreground
    //   so the lighter-weight call is enough — and we don't
    //   surprise the user by vanishing their main window when they
    //   pressed Esc to cancel.
    let app_for_main = app.clone();
    let picked = result.is_some();
    let _ = app.run_on_main_thread(move || {
        // Policy already Accessory (M5.5) — no toggle needed.
        let _ = &app_for_main;
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
        let png_path = match linkpilot_platform_mac::app_icon::ensure_png(bundle, path, name, 64) {
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
pub fn picker_resolve(state: tauri::State<'_, PickerState>, picked: Option<PickerPick>) {
    let sender_opt: Option<mpsc::Sender<Option<PickerPick>>> =
        state.pending.lock().ok().and_then(|mut g| g.take());
    if let Some(tx) = sender_opt {
        let _ = tx.send(picked);
    }
}

/// Open the picker in preview mode — same wheel, same keyboard, same
/// launch path. Settings → Appearance calls this so users can test their
/// picker style and profile ordering against their actual installed browsers
/// before depending on it during a real Ask flow.
///
/// Implementation: enumerate the user's browsers + apply their saved
/// profile order, hand it to `show_picker` from a worker thread, and launch
/// the supplied test URL with the resulting browser/profile. `show_picker`
/// blocks until the user picks or cancels — same lifecycle as a real ask.
#[tauri::command]
pub fn picker_preview(app: AppHandle, test_url: Option<String>) -> Result<(), String> {
    use linkpilot_core::browser::{apply_profile_order, BrowserTarget};

    let state: tauri::State<crate::state::AppState> = app.state();
    let installed = state
        .platform
        .browser_inventory()
        .installed_browsers()
        .map_err(|e| e.to_string())?;
    if installed.is_empty() {
        return Err("No browsers detected — install at least one to preview the picker.".into());
    }

    // Refuse to clobber a real ask. The picker state has a pending
    // sender if and only if there's a live Ask flow waiting for the
    // user; we'd corrupt that flow by stealing its window.
    let picker_state: tauri::State<PickerState> = app.state();
    if picker_state
        .pending
        .lock()
        .ok()
        .map(|g| g.is_some())
        .unwrap_or(false)
    {
        return Err("Another picker is already open — finish the current ask first.".into());
    }

    // Project out just the two settings we need; the rest of the
    // document stays behind the store mutex. Mirrors the same trick
    // dispatch::resolve_ask uses on its hot path.
    let (order_map, style) = state.config.with_document(|doc| {
        (
            doc.settings.profile_orders.clone(),
            doc.settings.picker_style,
        )
    });
    let inventory = state.platform.browser_inventory();
    let parsed_url = normalize_preview_url(test_url.as_deref())?;
    let url = parsed_url.to_string();

    let mut choices: Vec<PickerChoice> = Vec::with_capacity(installed.len());
    let mut target_by_id: HashMap<String, BrowserTarget> = HashMap::new();
    for b in &installed {
        let raw = inventory.profiles(&b.id).unwrap_or_default();
        let ordered = apply_profile_order(raw, order_map.get(&b.id.0).map(|v| v.as_slice()));
        let profiles: Vec<PickerProfile> = ordered
            .into_iter()
            .map(|p| PickerProfile {
                id: p.id,
                name: p.display_name,
                email: p.email,
                accent_color: p.accent_color,
                is_default: p.is_default,
            })
            .collect();
        let default_profile_id = profiles
            .iter()
            .find(|p| p.is_default)
            .or_else(|| profiles.first())
            .map(|p| p.id.clone());
        target_by_id.insert(b.id.0.clone(), BrowserTarget::new(b.id.clone()));
        choices.push(PickerChoice {
            id: b.id.0.clone(),
            name: b.display_name.clone(),
            bundle_id: b.platform_app_id.clone(),
            app_path: Some(app_path_from_executable(&b.executable)),
            icon_data_url: None,
            profiles,
            default_profile_id,
        });
    }

    let app_clone = app.clone();
    let state_clone = state.inner().clone();
    std::thread::spawn(move || {
        let picked = show_picker(&app_clone, &url, choices, style);
        let Some(picked) = picked else {
            tracing::info!("picker preview cancelled");
            return;
        };
        let Some(mut target) = target_by_id.remove(&picked.browser_id) else {
            tracing::warn!(?picked, "picker preview resolved unknown browser");
            return;
        };
        if let Some(profile_id) = picked.profile_id {
            target.profile = Some(profile_id);
        }
        tracing::info!(
            ?target,
            url = %parsed_url,
            "picker preview resolved — launching test URL"
        );
        if let Err(err) = state_clone
            .platform
            .url_launcher()
            .open(&target, &parsed_url)
        {
            tracing::error!(?err, ?target, url = %parsed_url, "picker preview launch failed");
        }
    });
    Ok(())
}

fn normalize_preview_url(test_url: Option<&str>) -> Result<url::Url, String> {
    let raw = test_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PREVIEW_URL);
    let candidate = if raw.contains("://") {
        raw.to_string()
    } else if is_local_preview_host(raw) {
        format!("http://{raw}")
    } else {
        format!("https://{raw}")
    };
    let parsed =
        url::Url::parse(&candidate).map_err(|err| format!("Invalid test URL '{raw}': {err}"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        scheme => Err(format!(
            "Invalid test URL '{raw}': unsupported scheme '{scheme}'"
        )),
    }
}

fn is_local_preview_host(raw: &str) -> bool {
    let host = raw
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(raw)
        .trim_start_matches('[');
    host == "localhost"
        || host.starts_with("localhost:")
        || host.starts_with("127.")
        || host.starts_with("0.0.0.0")
        || host.starts_with("::1")
}

fn app_path_from_executable(exe: &std::path::Path) -> String {
    // Same logic as dispatch.rs::app_path_from_executable — duplicated
    // here so picker.rs doesn't have to depend on dispatch internals.
    let s = exe.to_string_lossy();
    match s.rfind(".app/") {
        Some(idx) => s[..idx + 4].to_string(),
        None => s.into_owned(),
    }
}
