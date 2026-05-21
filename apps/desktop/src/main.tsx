import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { PickerWindow } from "./picker/PickerWindow";
import { TrayPopover } from "./tray/TrayPopover";
import { bootstrapTheme } from "./lib/theme";
import { bootstrapI18n } from "./i18n";
import "./styles/app.css";

// Initialize i18next before any React tree mounts. All three entry points
// (App, PickerWindow, TrayPopover) share the same i18n instance — picker
// and tray webviews each go through main.tsx, so this single call covers
// every window.
bootstrapI18n();

// Tauri renders the same frontendDist in every window; we pick which
// component tree to render based on a query parameter the Rust side
// sets when opening a non-main window.
//   ?view=picker  → src-tauri/src/picker.rs uses this for the URL chooser
//   ?view=tray    → reserved for the tray popover (Rust wiring pending)
const view = new URLSearchParams(window.location.search).get("view");
const isPicker = view === "picker";
const isTray = view === "tray";

// Apply the persisted theme on the main window only. Picker / tray follow
// the system appearance via the inline script in index.html so their
// backdrop and text contrast match the OS chrome — overriding that here
// re-introduces the light/dark mismatch (e.g. picker forced to "light"
// while the tray popover's vibrancy still tracks system dark mode).
if (!isPicker && !isTray) {
  bootstrapTheme();
}

// Belt-and-suspenders: the inline head script in index.html already
// added `picker-root` / `tray-root` synchronously to prevent a white
// flash. Keeping these here protects against a CSP misconfiguration
// that would prevent the inline script from running.
if (isPicker) {
  document.documentElement.classList.add("picker-root");
}
if (isTray) {
  document.documentElement.classList.add("tray-root");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isPicker ? <PickerWindow /> : isTray ? <TrayPopover /> : <App />}
  </React.StrictMode>,
);
