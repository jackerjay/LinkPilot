import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { PickerWindow } from "./picker/PickerWindow";
import { TrayPopover } from "./tray/TrayPopover";
import { bootstrapTheme } from "./lib/theme";
import "./styles/app.css";

// Tauri renders the same frontendDist in every window; we pick which
// component tree to render based on a query parameter the Rust side
// sets when opening a non-main window.
//   ?view=picker  → src-tauri/src/picker.rs uses this for the URL chooser
//   ?view=tray    → reserved for the tray popover (Rust wiring pending)
const view = new URLSearchParams(window.location.search).get("view");
const isPicker = view === "picker";
const isTray = view === "tray";

// Apply the persisted theme in every Tauri window. index.html does the same
// synchronously for picker/tray to avoid first-paint flashes; this keeps the
// runtime DOM in sync if the bootstrap path changes.
bootstrapTheme();

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
