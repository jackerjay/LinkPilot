// <AppIcon /> — show a macOS application's icon next to its name.
//
// Loads the icon via the Tauri `app_icon` command, which extracts the
// .icns from the .app and converts it to a 64pt PNG (cached on disk by
// the daemon). Results are also kept in a module-level Map so a single
// page rendering many rows doesn't fan out duplicate IPC calls.
//
// Falls back to a generic lucide icon while loading or on failure — the
// app stays usable even when LinkPilot can't resolve a particular bundle.

import { AppWindow } from "lucide-react";
import { useEffect, useState } from "react";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";

// `bundle_id|app_path|name` → data URL. `null` means "we tried and the
// daemon couldn't find one"; we cache that too so we don't keep retrying.
const cache = new Map<string, string | null>();
const inflight = new Map<string, Promise<string | null>>();

function cacheKey(
  bundleId?: string | null,
  appPath?: string | null,
  name?: string | null,
): string {
  return `${bundleId ?? ""}|${appPath ?? ""}|${name ?? ""}`;
}

async function load(
  bundleId?: string | null,
  appPath?: string | null,
  name?: string | null,
): Promise<string | null> {
  const key = cacheKey(bundleId, appPath, name);
  if (cache.has(key)) return cache.get(key) ?? null;
  const existing = inflight.get(key);
  if (existing) return existing;
  const p = (async () => {
    try {
      const result = await ipc.appIcon({
        bundle_id: bundleId ?? null,
        app_path: appPath ?? null,
        name: name ?? null,
      });
      const url = result?.data_url ?? null;
      cache.set(key, url);
      return url;
    } catch {
      cache.set(key, null);
      return null;
    } finally {
      inflight.delete(key);
    }
  })();
  inflight.set(key, p);
  return p;
}

interface Props {
  bundleId?: string | null;
  appPath?: string | null;
  /** Resolve the app via Spotlight by display name when neither
   *  bundleId nor appPath is known. Slowest of the three; only use it
   *  for surfaces (Rules list source-app matcher) that have no better
   *  identifier. */
  name?: string | null;
  /** Rendered CSS size (in CSS pixels). The Rust side always renders 64pt. */
  size?: number;
  className?: string;
  /** Optional alt text for accessibility. */
  alt?: string;
}

export function AppIcon({
  bundleId,
  appPath,
  name,
  size = 18,
  className,
  alt,
}: Props) {
  const [url, setUrl] = useState<string | null>(() =>
    cache.get(cacheKey(bundleId, appPath, name)) ?? null,
  );
  const [pending, setPending] = useState<boolean>(() => {
    const k = cacheKey(bundleId, appPath, name);
    return !cache.has(k);
  });

  useEffect(() => {
    if (!bundleId && !appPath && !name) {
      setUrl(null);
      setPending(false);
      return;
    }
    let alive = true;
    const k = cacheKey(bundleId, appPath, name);
    if (cache.has(k)) {
      setUrl(cache.get(k) ?? null);
      setPending(false);
      return;
    }
    setPending(true);
    load(bundleId, appPath, name).then((u) => {
      if (alive) {
        setUrl(u);
        setPending(false);
      }
    });
    return () => {
      alive = false;
    };
  }, [bundleId, appPath, name]);

  const style = { width: size, height: size } as const;

  if (url) {
    return (
      <img
        src={url}
        alt={alt ?? ""}
        style={style}
        className={cn("shrink-0 rounded-[4px]", className)}
      />
    );
  }
  return (
    <AppWindow
      style={style}
      className={cn(
        "shrink-0",
        pending ? "text-muted-foreground/40" : "text-muted-foreground",
        className,
      )}
      aria-label={alt}
    />
  );
}
