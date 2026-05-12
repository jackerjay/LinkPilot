//! Menu bar / system tray. Keeps the daemon alive when the main window is
//! closed and offers a quick Quit affordance.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show LinkPilot", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit LinkPilot", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
