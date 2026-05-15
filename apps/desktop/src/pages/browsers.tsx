import { useCallback, useEffect, useState } from "react";
import { Plus, RefreshCcw, Trash2, User } from "lucide-react";
import { AppIcon } from "@/components/AppIcon";
import { appPathFromExecutable } from "@/lib/browsers";
import { ipc } from "@/lib/ipc";
import type { BrowserProfile, InstalledBrowser } from "@/lib/types";

interface Entry {
  browser: InstalledBrowser;
  profiles: BrowserProfile[];
  error?: string;
  /** True when this entry came from `config.custom_browsers` rather
   *  than auto-detection. Drives the "custom" tag and remove button. */
  custom: boolean;
}

export function BrowsersPage() {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [installed, doc] = await Promise.all([
        ipc.listBrowsers(),
        ipc.configGet(),
      ]);
      // Custom ids set; the merged list from list_browsers contains
      // both detected and custom entries, so we tag by membership in
      // doc.custom_browsers rather than guessing from `kind: unknown`.
      const customIds = new Set(
        (doc.custom_browsers ?? []).map((b) => b.id),
      );
      const out: Entry[] = await Promise.all(
        installed.map(async (b) => {
          try {
            return {
              browser: b,
              profiles: await ipc.listProfiles(b.id),
              custom: customIds.has(b.id),
            };
          } catch (err) {
            return {
              browser: b,
              profiles: [],
              error: String(err),
              custom: customIds.has(b.id),
            };
          }
        }),
      );
      setEntries(out);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const addManually = async () => {
    setBusy(true);
    setError(null);
    try {
      const picked = await ipc.pickApp();
      if (!picked) {
        // User cancelled — silent return.
        setBusy(false);
        return;
      }
      // Build an InstalledBrowser from the picker result. We choose
      // `kind: "unknown"` deliberately — the launcher routes Unknown
      // through `open -a <display_name>` which works for any LSHandlable
      // app without needing to know the binary name inside the bundle.
      // Profile enumeration is skipped for Unknown.
      //
      // id: prefer bundle_id (stable across versions); fall back to a
      // slugified name when the app has no bundle id (rare — usually
      // unsigned dev builds).
      const id =
        picked.bundle_id ||
        picked.name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      // executable: use the .app path so `open -a` and AppIcon both
      // work; the launcher's Unknown branch ignores it but the field
      // is required by InstalledBrowser.
      const executable = picked.app_path || `/Applications/${picked.name}.app`;
      const browser: InstalledBrowser = {
        id,
        display_name: picked.name,
        kind: "unknown",
        executable,
        platform_app_id: picked.bundle_id || null,
        profile_root: null,
      };
      await ipc.addCustomBrowser(browser);
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const removeCustom = async (id: string) => {
    setError(null);
    try {
      await ipc.removeCustomBrowser(id);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  if (error && entries === null) {
    return (
      <div>
        <h2 className="mac-h2">Browsers</h2>
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag danger">error</span>
            <span className="grow mac-muted">{error}</span>
          </div>
        </div>
      </div>
    );
  }

  const totalProfiles =
    entries?.reduce((n, e) => n + e.profiles.length, 0) ?? 0;

  return (
    <div>
      <h2 className="mac-h2">Browsers</h2>
      <p className="mac-subtitle">
        Detected browsers and the profiles LinkPilot can route to. Add a
        custom application if a browser isn't auto-detected.
      </p>

      {/* Action bar */}
      <div
        style={{
          display: "flex",
          gap: 8,
          alignItems: "center",
          marginBottom: 12,
        }}
      >
        <button
          type="button"
          className="mac-tbtn"
          onClick={() => {
            setEntries(null);
            refresh();
          }}
          disabled={busy}
        >
          <RefreshCcw size={12} strokeWidth={1.8} />
          <span>Rescan</span>
        </button>
        <button
          type="button"
          className="mac-tbtn"
          onClick={addManually}
          disabled={busy}
        >
          <Plus size={12} strokeWidth={2} />
          <span>{busy ? "Choosing…" : "Add manually"}</span>
        </button>
        <span style={{ flex: 1 }} />
        {entries !== null && entries.length > 0 && (
          <span className="mac-muted" style={{ fontSize: 12 }}>
            {entries.length} browsers · {totalProfiles} profiles
          </span>
        )}
      </div>

      {error && (
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag danger">error</span>
            <span className="grow mac-muted">{error}</span>
          </div>
        </div>
      )}

      {entries === null ? (
        <div className="mac-card">
          <div
            className="mac-row mac-muted"
            style={{ justifyContent: "center", padding: "24px 18px" }}
          >
            Scanning…
          </div>
        </div>
      ) : entries.length === 0 ? (
        <div className="mac-card">
          <div
            className="mac-row mac-muted"
            style={{ justifyContent: "center", padding: "24px 18px" }}
          >
            No browsers detected. Try{" "}
            <span className="mac-mono" style={{ margin: "0 4px" }}>
              Add manually
            </span>{" "}
            to register one yourself.
          </div>
        </div>
      ) : (
        <div className="mac-card">
          {entries.map((e) => (
            <BrowserBlock
              key={e.browser.id}
              entry={e}
              onRemove={() => removeCustom(e.browser.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function BrowserBlock({
  entry: e,
  onRemove,
}: {
  entry: Entry;
  onRemove: () => void;
}) {
  return (
    <>
      <div
        className="mac-row"
        style={{ alignItems: "flex-start", paddingTop: 14, paddingBottom: 14 }}
      >
        <AppIcon
          bundleId={e.browser.platform_app_id ?? undefined}
          appPath={appPathFromExecutable(e.browser.executable)}
          size={32}
          alt={e.browser.display_name}
          className="shrink-0"
        />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <strong style={{ fontSize: 13.5 }}>{e.browser.display_name}</strong>
            <span className="mac-muted" style={{ fontSize: 12 }}>
              {e.browser.kind}
            </span>
            {e.custom && (
              <span
                className="mac-tag neutral"
                title="Added manually — remove with the trash button"
              >
                custom
              </span>
            )}
          </div>
          <div
            className="select-text mac-mono mac-muted"
            style={{ fontSize: 11, marginTop: 2, wordBreak: "break-all" }}
          >
            {e.browser.executable}
          </div>
          {e.error && (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                marginTop: 6,
              }}
            >
              <span className="mac-tag danger">profiles</span>
              <span className="mac-muted" style={{ fontSize: 11.5 }}>
                {e.error}
              </span>
            </div>
          )}
        </div>
        {e.custom && (
          <button
            type="button"
            className="mac-tbtn"
            onClick={onRemove}
            title={`Remove "${e.browser.display_name}" from custom browsers`}
            style={{ color: "var(--mac-danger)" }}
          >
            <Trash2 size={13} strokeWidth={1.8} />
          </button>
        )}
      </div>
      {e.profiles.map((p) => (
        <div key={p.id} className="mac-row" style={{ paddingLeft: 62 }}>
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <User size={14} strokeWidth={1.8} />
          </span>
          <div className="grow">
            <div style={{ fontSize: 12.5 }}>{p.display_name}</div>
            {p.email && (
              <div className="mac-muted" style={{ fontSize: 11.5 }}>
                {p.email}
              </div>
            )}
          </div>
          <span className="mac-mono mac-muted" style={{ fontSize: 11 }}>
            {p.id}
          </span>
        </div>
      ))}
    </>
  );
}
