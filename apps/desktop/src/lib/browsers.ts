// Shared browser-inventory helpers.
//
// The inventory rarely changes during a session (browsers don't get
// installed every minute), so we keep the list in a module-level cache
// after the first ipc call. Every <BrowserBadge> that needs to look up a
// display name + bundle id can mount cheaply without each page having to
// thread `browsers` through props.

import { useEffect, useState } from "react";
import { ipc } from "@/lib/ipc";
import type { InstalledBrowser } from "@/lib/types";

interface Cache {
  data: InstalledBrowser[] | null;
  pending: Promise<InstalledBrowser[]> | null;
  subscribers: Set<(list: InstalledBrowser[]) => void>;
}

const cache: Cache = { data: null, pending: null, subscribers: new Set() };

function fetchOnce(): Promise<InstalledBrowser[]> {
  if (cache.data) return Promise.resolve(cache.data);
  if (cache.pending) return cache.pending;
  cache.pending = ipc
    .listBrowsers()
    .catch(() => [] as InstalledBrowser[])
    .then((list) => {
      cache.data = list;
      cache.pending = null;
      cache.subscribers.forEach((cb) => cb(list));
      return list;
    });
  return cache.pending;
}

/// Subscribe to the inventory. Always returns synchronously: empty array
/// before the first fetch resolves, then the real list. Re-renders happen
/// automatically when the fetch completes.
export function useBrowsers(): InstalledBrowser[] {
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>(
    cache.data ?? [],
  );
  useEffect(() => {
    if (cache.data) {
      setBrowsers(cache.data);
      return;
    }
    cache.subscribers.add(setBrowsers);
    fetchOnce().catch(() => {
      /* setBrowsers already covered by subscriber notification */
    });
    return () => {
      cache.subscribers.delete(setBrowsers);
    };
  }, []);
  return browsers;
}

/// `/Applications/Foo.app/Contents/MacOS/Foo` → `/Applications/Foo.app`.
/// Used by AppIcon so the icon resolver doesn't need the bundle id for
/// browsers (we always have the .app path via InstalledBrowser.executable).
export function appPathFromExecutable(executable: string): string {
  const idx = executable.lastIndexOf(".app/");
  if (idx === -1) return executable;
  return executable.slice(0, idx + 4);
}

/// Best-effort human label for a browser id. Returns the inventory's
/// display_name when known, otherwise capitalises the id ("arc" → "Arc").
export function browserDisplayName(
  id: string,
  inventory: InstalledBrowser[],
): string {
  const found = inventory.find((b) => b.id === id);
  if (found) return found.display_name;
  return id.charAt(0).toUpperCase() + id.slice(1);
}
