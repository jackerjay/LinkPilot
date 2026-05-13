// Theme management. Three modes:
//   - "system": follows macOS Appearance via prefers-color-scheme
//   - "light" / "dark": manual override
//
// Persisted in localStorage so we don't need a Rust round-trip on boot.
// Applied by toggling `data-theme="light"|"dark"` on <html>; CSS variables
// in styles/app.css read off that attribute.

import { useEffect, useState } from "react";

export type ThemeMode = "system" | "light" | "dark";

const STORAGE_KEY = "linkpilot.theme";

export function readStoredMode(): ThemeMode {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw === "light" || raw === "dark" || raw === "system") return raw;
  return "system";
}

function resolveActive(mode: ThemeMode): "light" | "dark" {
  if (mode !== "system") return mode;
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function apply(mode: ThemeMode) {
  const active = resolveActive(mode);
  const root = document.documentElement;
  root.setAttribute("data-theme", active);
  // shadcn/ui consumes a `.dark` class on <html> for the dark token set.
  // Keep both in sync so the design tokens (Tailwind) and any legacy
  // [data-theme] selectors agree.
  root.classList.toggle("dark", active === "dark");
}

/// Subscribe to mode changes + the system media query if mode === "system".
export function useTheme(): {
  mode: ThemeMode;
  active: "light" | "dark";
  setMode: (next: ThemeMode) => void;
} {
  const [mode, setModeState] = useState<ThemeMode>(() => readStoredMode());
  const [active, setActive] = useState<"light" | "dark">(() =>
    resolveActive(mode),
  );

  useEffect(() => {
    apply(mode);
    setActive(resolveActive(mode));
    if (mode !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      apply(mode);
      setActive(resolveActive(mode));
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [mode]);

  const setMode = (next: ThemeMode) => {
    localStorage.setItem(STORAGE_KEY, next);
    setModeState(next);
  };

  return { mode, active, setMode };
}

/// Bootstrap on first paint so we don't get a light-flash before React mounts.
/// Called from main.tsx, before render.
export function bootstrapTheme(): void {
  apply(readStoredMode());
}
