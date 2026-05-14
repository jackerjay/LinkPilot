import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { PickerWindow } from "./picker/PickerWindow";
import { bootstrapTheme } from "./lib/theme";
import "./styles/app.css";

// Apply the persisted theme before the first React render so the user
// never sees a light flash on dark-mode boot.
bootstrapTheme();

// Tauri renders the same frontendDist in every window; we pick which
// component tree to render based on a query parameter the Rust side
// sets when opening the picker window (see src-tauri/src/picker.rs).
const isPicker = new URLSearchParams(window.location.search).get("view") === "picker";

// The picker window relies on a transparent webview so the
// NSVisualEffectView (vibrancy) behind it shows through. Without this
// class the global html/body background from app.css would be opaque
// and the blur would be invisible.
if (isPicker) {
  document.documentElement.classList.add("picker-root");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{isPicker ? <PickerWindow /> : <App />}</React.StrictMode>,
);
