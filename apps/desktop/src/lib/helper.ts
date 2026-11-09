import { useCallback, useEffect, useState } from "react";
import { isAppStore } from "./distribution";
import { ipc, type HelperStatus } from "./ipc";

export const HELPER_CHANGED = "linkpilot-helper-changed";

export function useHelper() {
  const [status, setStatus] = useState<HelperStatus | null>(null);
  const refresh = useCallback(async () => {
    if (!isAppStore) return;
    try {
      setStatus(await ipc.helperStatus());
    } catch (error) {
      setStatus({ connected: false, version: null, profile_browsers: [], error: String(error) });
    }
  }, []);
  useEffect(() => {
    void refresh();
    const changed = () => void refresh();
    window.addEventListener(HELPER_CHANGED, changed);
    window.addEventListener("focus", changed);
    return () => {
      window.removeEventListener(HELPER_CHANGED, changed);
      window.removeEventListener("focus", changed);
    };
  }, [refresh]);
  return { status, refresh, supports: (browser: string) =>
    !isAppStore || Boolean(status?.connected && status.profile_browsers.includes(browser)) };
}
