// Cmd-Tab-style browser chooser for routes whose action is `ask`.
// Lives in its own Tauri window (label="picker"); see src-tauri/src/picker.rs
// for the lifecycle. Loads the session via picker_session(), submits the
// pick via picker_resolve(id|null).
//
// Keyboard:
//   ← / →    move highlight (wraps)
//   Enter   confirm
//   Esc     cancel

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppIcon } from "@/components/AppIcon";
import { cn } from "@/lib/utils";

interface PickerChoice {
  id: string;
  name: string;
  bundle_id?: string | null;
  app_path?: string | null;
  /** Base64-encoded PNG (`data:image/png;base64,…`) pre-rendered by
   *  the Rust side. When present, the renderer paints it directly
   *  instead of issuing an `app_icon` ipc round-trip — kills the
   *  blank → real icon flash on every Ask. */
  icon_data_url?: string | null;
}

interface PickerSession {
  url: string;
  choices: PickerChoice[];
}

function resolve(picked: string | null) {
  // fire-and-forget — Rust closes the window once the channel signals.
  invoke("picker_resolve", { picked }).catch(() => {});
}

export function PickerWindow() {
  const [session, setSession] = useState<PickerSession | null>(null);
  const [selected, setSelected] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Belt-and-suspenders: force every layer above the
    // NSVisualEffectView to transparent at runtime so the vibrancy
    // actually reaches the user's eye. The picker-root CSS class
    // does the same but inline style always wins.
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    const root = document.getElementById("root");
    if (root) root.style.background = "transparent";

    invoke<PickerSession | null>("picker_session")
      .then((s) => setSession(s))
      .catch(() => setSession(null));
  }, []);

  useEffect(() => {
    containerRef.current?.focus();
  }, [session]);

  const choices = session?.choices ?? [];
  const n = choices.length;

  const pick = useCallback(
    (idx: number) => {
      const c = choices[idx];
      if (c) resolve(c.id);
    },
    [choices],
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (n === 0) return;
      switch (e.key) {
        case "ArrowRight":
        case "Tab":
          e.preventDefault();
          setSelected((s) => (s + 1) % n);
          break;
        case "ArrowLeft":
          e.preventDefault();
          setSelected((s) => (s - 1 + n) % n);
          break;
        case "Enter":
          e.preventDefault();
          pick(selected);
          break;
        case "Escape":
          e.preventDefault();
          resolve(null);
          break;
      }
    },
    [n, selected, pick],
  );

  const urlPreview = useMemo(() => {
    if (!session) return "";
    // Show the first 80 chars + ellipsis if longer. Multi-line break-all
    // would push the icon row off-screen on long query strings.
    return session.url.length > 80
      ? session.url.slice(0, 80) + "…"
      : session.url;
  }, [session]);

  return (
    <div
      ref={containerRef}
      tabIndex={0}
      onKeyDown={onKeyDown}
      // macOS-transparent window with NSVisualEffectView vibrancy under
      // the webview (see picker.rs::apply_glass). Let the vibrancy
      // carry the entire background — `bg-black/X` on top muddies the
      // blur. `outline-none` kills the WebKit focus ring on the
      // tabIndex=0 container that read as an unintentional border.
      //
      // Text colours use the app's theme tokens (--foreground /
      // --muted-foreground) so the labels stay legible on BOTH the
      // light-mode HudWindow (light backdrop → dark text) and the
      // dark-mode HudWindow (dark backdrop → near-white text). The
      // earlier hard-coded `text-white` / `text-neutral-*` were
      // designed for dark backdrops only and disappeared in light mode.
      className="flex h-screen flex-col items-center justify-center gap-5 rounded-2xl p-6 text-foreground outline-none focus:outline-none"
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div className="flex w-full flex-col items-center gap-1">
        <div className="text-xs font-medium uppercase tracking-widest text-muted-foreground">
          Open with
        </div>
        <div
          className="select-text font-mono text-xs text-foreground/80"
          style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
          title={session?.url}
        >
          {urlPreview || "…"}
        </div>
      </div>

      <div
        className="flex items-center justify-center gap-2"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        {choices.map((c, idx) => (
          <button
            key={c.id}
            type="button"
            onClick={() => pick(idx)}
            onMouseEnter={() => setSelected(idx)}
            className={cn(
              "flex w-[88px] flex-col items-center gap-2 rounded-xl px-2 py-3 transition-colors",
              // Selection ring: use foreground-toned overlays so
              // contrast works in both light and dark vibrancy.
              idx === selected
                ? "bg-foreground/10 ring-1 ring-foreground/25"
                : "hover:bg-foreground/5",
            )}
          >
            {c.icon_data_url ? (
              // Pre-rendered icon: paint immediately, no async ipc.
              // shrink-0 matches AppIcon's class so the layout is
              // identical to the fallback path below.
              <img
                src={c.icon_data_url}
                alt={c.name}
                width={56}
                height={56}
                className="h-14 w-14 shrink-0 rounded-xl"
              />
            ) : (
              <AppIcon
                bundleId={c.bundle_id ?? undefined}
                appPath={c.app_path ?? undefined}
                name={c.name}
                size={56}
                alt={c.name}
                className="rounded-xl"
              />
            )}
            <span className="line-clamp-1 text-xs font-medium text-foreground">
              {c.name}
            </span>
          </button>
        ))}
      </div>

      <div
        className="text-[10px] font-medium uppercase tracking-widest text-muted-foreground"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        ← → select   ⏎ open   esc cancel
      </div>
    </div>
  );
}
