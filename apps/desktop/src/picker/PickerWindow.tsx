// Browser-pick UI for routes whose action is `ask`. Replaces the old
// Cmd-Tab-style tile row with the Halo wheel:
//
//   • Row of browser tiles (same as before).
//   • Click any tile → launch its default profile.
//   • Hold ⌥ over a multi-profile tile → portal-rendered 360° wheel.
//   • Aim with the mouse / press 1-9 / ⏎ / Esc to resolve.
//
// Lives in its own Tauri window (label="picker"); see src-tauri/src/picker.rs
// for the lifecycle. Loads the session via `picker_session`, submits the
// pick via `picker_resolve({ browser_id, profile_id })`.

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HaloShell } from "./halo/HaloShell";
import { HaloFrostedPortal } from "./halo/HaloFrosted";
import { HaloBezelPortal } from "./halo/HaloBezel";
import { HaloCrownPortal } from "./halo/HaloCrown";
import type { PickerPick, PickerSession } from "./halo/types";

import "./picker.css";

function resolve(picked: PickerPick | null) {
  // Fire-and-forget — Rust closes the window once the channel signals.
  invoke("picker_resolve", { picked }).catch(() => {});
}

export function PickerWindow() {
  const [session, setSession] = useState<PickerSession | null>(null);

  useEffect(() => {
    // Keep every webview layer transparent; the picker paints its own
    // constrained glass instead of relying on a full-window AppKit material.
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    const root = document.getElementById("root");
    if (root) root.style.background = "transparent";

    invoke<PickerSession | null>("picker_session")
      .then((s) => setSession(s))
      .catch(() => setSession(null));
  }, []);

  useEffect(() => {
    if (!session) return;
    const prepaint = document.getElementById("picker-prepaint");
    if (!prepaint) return;

    let removeTimer = 0;
    const frame = window.requestAnimationFrame(() => {
      prepaint.classList.add("leaving");
      removeTimer = window.setTimeout(() => prepaint.remove(), 220);
    });

    return () => {
      window.cancelAnimationFrame(frame);
      if (removeTimer) window.clearTimeout(removeTimer);
    };
  }, [session]);

  const onPick = useCallback(
    (browser_id: string, profile_id: string | null) => {
      resolve({ browser_id, profile_id });
    },
    [],
  );

  const onCancel = useCallback(() => {
    resolve(null);
  }, []);

  if (!session) {
    return (
      <div
        className="picker-root preparing"
        style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
      />
    );
  }

  const portalForStyle = (() => {
    switch (session.style) {
      case "bezel":
        return HaloBezelPortal;
      case "crown":
        return HaloCrownPortal;
      case "frosted":
      default:
        return HaloFrostedPortal;
    }
  })();

  // Crown owns its center display — suppress the floating readout to
  // avoid duplicate "this is what you're aiming at" UIs.
  const showReadout = session.style !== "crown";

  return (
    <div
      className="picker-root"
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        <HaloShell
          choices={session.choices}
          url={session.url}
          onPick={onPick}
          onCancel={onCancel}
          renderPortal={(args) => portalForStyle(args)}
          showReadout={showReadout}
        />
      </div>
    </div>
  );
}
