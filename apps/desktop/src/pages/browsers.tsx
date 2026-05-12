import { useEffect, useState } from "react";
import { ipc } from "../lib/ipc";
import type { BrowserProfile, InstalledBrowser } from "../lib/types";

interface Entry {
  browser: InstalledBrowser;
  profiles: BrowserProfile[];
  error?: string;
}

export function BrowsersPage() {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const installed = await ipc.listBrowsers();
        const out: Entry[] = await Promise.all(
          installed.map(async (b) => {
            try {
              return { browser: b, profiles: await ipc.listProfiles(b.id) };
            } catch (err) {
              return { browser: b, profiles: [], error: String(err) };
            }
          }),
        );
        setEntries(out);
      } catch (err) {
        setError(String(err));
      }
    })();
  }, []);

  if (error) {
    return (
      <>
        <h2>Browsers</h2>
        <div className="card">
          <span className="tag danger">error</span>
          <span className="muted"> {error}</span>
        </div>
      </>
    );
  }

  return (
    <>
      <h2>Browsers</h2>
      <p className="subtitle">
        Detected browsers and the profiles LinkPilot can route to.
      </p>

      {entries === null ? (
        <div className="empty">Scanning…</div>
      ) : entries.length === 0 ? (
        <div className="card empty">
          No browsers detected. (Are you running this from a sandbox with no{" "}
          <span className="mono">/Applications</span>?)
        </div>
      ) : (
        entries.map((e) => (
          <div key={e.browser.id} className="card">
            <div className="row">
              <span className="grow">
                <strong>{e.browser.display_name}</strong>{" "}
                <span className="muted mono">{e.browser.id}</span>
              </span>
              <span className="tag">{e.browser.kind}</span>
            </div>
            <div className="row">
              <span className="muted grow mono">{e.browser.executable}</span>
            </div>
            {e.error && (
              <div className="row">
                <span className="tag danger">profiles</span>
                <span className="muted">{e.error}</span>
              </div>
            )}
            {e.profiles.length > 0 && (
              <>
                <div className="row muted">Profiles</div>
                {e.profiles.map((p) => (
                  <div key={p.id} className="row">
                    <span className="grow">
                      {p.display_name}
                      {p.email && (
                        <span className="muted"> &middot; {p.email}</span>
                      )}
                    </span>
                    <span className="mono muted">{p.id}</span>
                  </div>
                ))}
              </>
            )}
          </div>
        ))
      )}
    </>
  );
}
