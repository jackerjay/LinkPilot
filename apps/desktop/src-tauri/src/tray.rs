//! Menu bar / system tray.
//!
//! Two responsibilities:
//!   1. Keep LinkPilot alive in the menu bar when the main window is
//!      closed; offer a Quit affordance.
//!   2. Show a Spotlight/Raycast-style popover anchored under the tray
//!      icon when the user left-clicks it (Cmd-Tab-style quick-access
//!      surface for routes, workspaces, recent activity). The popover
//!      is a separate Tauri webview window labelled `tray`; the
//!      frontend dispatches on `?view=tray` (see main.tsx) to render
//!      `<TrayPopover>` instead of `<App>`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use linkpilot_core::config::LanguagePref;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, Runtime, WebviewUrl, WebviewWindowBuilder,
};

const TRAY_LABEL: &str = "tray";
const POPOVER_W: f64 = 360.0;
const POPOVER_H: f64 = 500.0;
/// Gap between the menu-bar icon's bottom edge and the popover's top
/// edge, in logical pixels. ~8px is what Bartender / Stats use.
const ANCHOR_GAP: f64 = 8.0;
/// Suppress the next show-on-click for this long after a focus-lost
/// hide. Without this, clicking the tray icon to dismiss the popover
/// would fire focus-lost → hide → click → re-show, because focus-lost
/// arrives *before* the click handler on macOS.
const REOPEN_SUPPRESS_MS: u64 = 250;
const MAIN_TRAY_ID: &str = "main-tray";

/// Cross-thread bookkeeping for the popover's click-toggle behavior.
/// `app.manage`-d so the focus-lost handler in `lib.rs` and the
/// tray-click handler here read the same source of truth.
#[derive(Default)]
pub struct TrayState {
    last_hidden_at: Mutex<Option<Instant>>,
}

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.manage(TrayState::default());

    let language = app
        .try_state::<crate::state::AppState>()
        .map(|state| state.config.document().settings.language)
        .unwrap_or_default();
    let menu = build_tray_menu(app, language)?;

    let icon_bytes = include_bytes!("../icons/tray.png");
    let icon = Image::from_bytes(icon_bytes)
        .map_err(|e| tauri::Error::AssetNotFound(format!("tray icon decode: {e}")))?;

    let _tray = TrayIconBuilder::with_id(MAIN_TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        // Left-click is our popover trigger; right-click (or long-press)
        // surfaces the Show/Quit menu. Without this the left-click would
        // also drop the menu, breaking the toggle behavior.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                handle_left_click(tray.app_handle(), rect);
            }
        })
        .build(app)?;
    Ok(())
}

pub fn update_menu_language<R: Runtime>(app: &AppHandle<R>, language: LanguagePref) {
    let Some(tray) = app.tray_by_id(MAIN_TRAY_ID) else {
        return;
    };
    let menu = match build_tray_menu(app, language) {
        Ok(menu) => menu,
        Err(err) => {
            tracing::warn!(?err, "tray: localized menu build failed");
            return;
        }
    };
    if let Err(err) = tray.set_menu(Some(menu)) {
        tracing::warn!(?err, "tray: localized menu update failed");
    }
}

fn build_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    language: LanguagePref,
) -> tauri::Result<Menu<R>> {
    let labels = tray_menu_labels(language);
    let show = MenuItem::with_id(app, "show", labels.show, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&show, &quit])
}

struct TrayMenuLabels {
    show: &'static str,
    quit: &'static str,
}

fn tray_menu_labels(language: LanguagePref) -> TrayMenuLabels {
    match language {
        LanguagePref::ZhCn => TrayMenuLabels {
            show: "显示 LinkPilot",
            quit: "退出 LinkPilot",
        },
        LanguagePref::ZhTw => TrayMenuLabels {
            show: "顯示 LinkPilot",
            quit: "結束 LinkPilot",
        },
        LanguagePref::JaJp => TrayMenuLabels {
            show: "LinkPilot を表示",
            quit: "LinkPilot を終了",
        },
        LanguagePref::System | LanguagePref::En => TrayMenuLabels {
            show: "Show LinkPilot",
            quit: "Quit LinkPilot",
        },
    }
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    // Hide the popover too — the user is moving into the full app, no
    // reason to leave a stale ghost popover hovering over the menu bar.
    if let Some(popover) = app.get_webview_window(TRAY_LABEL) {
        let _ = popover.hide();
    }
}

/// Frontend → Rust bridge for the popover's affordances that surface
/// the main app. Optional `tab` argument deep-links to a specific tab
/// (Overview / Rules / Inspector / Browsers / Settings); the main App
/// listens on `tray:navigate` and calls its internal setTab. Omitting
/// `tab` is just "show me the app" with no view switch.
#[tauri::command]
pub fn tray_open_main(app: AppHandle, tab: Option<String>) {
    show_main_window(&app);
    if let Some(tab) = tab {
        // Best-effort emit. Frontend coalesces multiple events
        // naturally (each setTab is idempotent) so we don't need to
        // worry about delivery order.
        if let Err(err) = app.emit("tray:navigate", tab) {
            tracing::warn!(?err, "tray: navigate emit failed");
        }
    }
}

fn handle_left_click<R: Runtime>(app: &AppHandle<R>, rect: tauri::Rect) {
    let state: tauri::State<TrayState> = app.state();
    let recently_hidden = state
        .last_hidden_at
        .lock()
        .ok()
        .and_then(|g| *g)
        .is_some_and(|t| t.elapsed() < Duration::from_millis(REOPEN_SUPPRESS_MS));
    if recently_hidden {
        // The click that's reaching us here is the SAME mouse-down that
        // moved focus away from the popover, which already triggered a
        // hide via the focus-lost handler in lib.rs. Treat as a no-op
        // so the popover stays dismissed.
        return;
    }

    // Existing window? Toggle.
    if let Some(win) = app.get_webview_window(TRAY_LABEL) {
        let visible = win.is_visible().unwrap_or(false);
        if visible {
            let _ = win.hide();
            stamp_hidden(&state);
            return;
        }
        // Window exists but is hidden — reposition (the user may have
        // dragged the menu bar to another display) and show.
        if let Some((x, y)) = popover_position(app, rect) {
            let _ = win.set_position(LogicalPosition::new(x, y));
        }
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    // Fresh build. WebviewWindow construction is cheap on macOS (the
    // process already has WKWebView linked) so we don't bother
    // pre-creating it — the first click is still snappy.
    build_and_show_popover(app, rect);
}

fn build_and_show_popover<R: Runtime>(app: &AppHandle<R>, rect: tauri::Rect) {
    let target = WebviewUrl::App("index.html?view=tray".into());
    let mut builder = WebviewWindowBuilder::new(app, TRAY_LABEL, target)
        .title("LinkPilot")
        .inner_size(POPOVER_W, POPOVER_H)
        .min_inner_size(POPOVER_W, POPOVER_H)
        .max_inner_size(POPOVER_W, POPOVER_H)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .skip_taskbar(true)
        // Build invisible so vibrancy + collection-behavior land before
        // the first paint (avoids the white-flash → frosted transition
        // we hit with the picker too).
        .visible(false)
        .focused(true);

    match popover_position(app, rect) {
        Some((x, y)) => builder = builder.position(x, y),
        None => builder = builder.center(),
    }

    let win = match builder.build() {
        Ok(w) => w,
        Err(err) => {
            tracing::warn!(?err, "tray: popover build failed");
            return;
        }
    };

    // AppKit calls (vibrancy + collection behavior) MUST run on the
    // main thread — picker.rs uses the same pattern. We're already on
    // the main thread here (tray callback dispatched by Tauri's run
    // loop) but use run_on_main_thread anyway for symmetry and to
    // future-proof against the tray runtime moving off-main.
    let win_for_main = win.clone();
    let _ = app.run_on_main_thread(move || {
        apply_glass(&win_for_main);
        elevate_collection_behavior(&win_for_main);
        let _ = win_for_main.show();
        let _ = win_for_main.set_focus();
    });
}

/// Map the tray icon's frame to the top-left logical position of the
/// popover, centered horizontally below the icon.
///
/// `Rect.position` and `.size` are `dpi::Position`/`dpi::Size` enums
/// (Logical | Physical variants). On macOS the tray subsystem reports
/// physical pixels, but we normalize via `to_physical(1.0)` so the
/// same code path works if a future Tauri release flips the
/// convention — the conversion is a no-op for the Physical variant.
fn popover_position<R: Runtime>(app: &AppHandle<R>, rect: tauri::Rect) -> Option<(f64, f64)> {
    let icon_phys = rect.position.to_physical::<f64>(1.0);
    let icon_size = rect.size.to_physical::<f64>(1.0);
    let icon_x = icon_phys.x;
    let icon_y = icon_phys.y;
    let icon_w = icon_size.width;
    let icon_h = icon_size.height;

    let monitors = app.available_monitors().ok()?;
    let monitor = monitors.into_iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        let px = pos.x as f64;
        let py = pos.y as f64;
        icon_x >= px
            && icon_x < px + size.width as f64
            && icon_y >= py
            && icon_y < py + size.height as f64
    })?;
    let scale = monitor.scale_factor();
    let icon_center_x_logical = (icon_x + icon_w / 2.0) / scale;
    let icon_bottom_y_logical = (icon_y + icon_h) / scale;

    let monitor_x_logical = monitor.position().x as f64 / scale;
    let monitor_w_logical = monitor.size().width as f64 / scale;

    let mut x = icon_center_x_logical - POPOVER_W / 2.0;
    let y = icon_bottom_y_logical + ANCHOR_GAP;

    // Clamp horizontally so the popover never spills past the screen
    // edge. 8px breathing margin matches macOS' own behavior.
    let min_x = monitor_x_logical + 8.0;
    let max_x = monitor_x_logical + monitor_w_logical - POPOVER_W - 8.0;
    if x < min_x {
        x = min_x;
    }
    if x > max_x {
        x = max_x;
    }
    Some((x, y))
}

fn stamp_hidden(state: &tauri::State<TrayState>) {
    if let Ok(mut g) = state.last_hidden_at.lock() {
        *g = Some(Instant::now());
    }
}

/// Public so `lib.rs`'s focus-lost handler can stamp the timestamp
/// after auto-hiding the popover.
pub fn note_popover_hidden<R: Runtime>(app: &AppHandle<R>) {
    let state: tauri::State<TrayState> = app.state();
    stamp_hidden(&state);
}

fn apply_glass<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        // HudWindow = Spotlight / Raycast / Cmd-Tab vibrancy. Same
        // material the picker uses for consistency. 14.0 corner radius
        // matches `.mac-popover` in app.css so the vibrancy doesn't
        // bleed outside the inner panel's rounded corners.
        if let Err(err) = apply_vibrancy(
            window,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::Active),
            Some(14.0),
        ) {
            tracing::warn!(?err, "tray: vibrancy failed");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }
}

fn elevate_collection_behavior<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    // Show on all spaces (including fullscreen) so a fullscreen Lark
    // or browser doesn't hide the menu-bar popover. Same playbook as
    // the picker's `elevate_above_fullscreen`.
    if let Err(err) = window.set_visible_on_all_workspaces(true) {
        tracing::warn!(?err, "tray: set_visible_on_all_workspaces failed");
    }

    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;

        // CanJoinAllSpaces (1) + FullScreenAuxiliary (1<<8=256)
        // + Stationary (1<<4=16) so it doesn't slide into Mission
        // Control thumbnails. NSPopUpMenuWindowLevel = 101 sits above
        // fullscreen app chrome.
        const COLLECTION_BEHAVIOR: usize = 1 | (1 << 8) | (1 << 4);
        const NS_POPUP_MENU_WINDOW_LEVEL: isize = 101;

        let raw = match window.ns_window() {
            Ok(p) => p as *mut AnyObject,
            Err(err) => {
                tracing::warn!(?err, "tray: ns_window unavailable");
                return;
            }
        };
        if raw.is_null() {
            return;
        }
        unsafe {
            let ns: &AnyObject = &*raw;
            let _: () = msg_send![ns, setCollectionBehavior: COLLECTION_BEHAVIOR];
            let _: () = msg_send![ns, setLevel: NS_POPUP_MENU_WINDOW_LEVEL];
        }
    }
}
