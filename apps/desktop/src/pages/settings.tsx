import { useCallback, useEffect, useState } from "react";
import { TargetEditor } from "../components/TargetEditor";
import { ipc } from "../lib/ipc";
import { useTheme, type ThemeMode } from "../lib/theme";
import type {
  BrowserTarget,
  ConfigDocument,
  InstalledBrowser,
  SetDefaultOutcome,
} from "../lib/types";

interface Props {
  configEpoch: number;
}

export function SettingsPage({ configEpoch }: Props) {
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [configPath, setConfigPath] = useState<string | null>(null);
  const [isDefault, setIsDefault] = useState<boolean | null>(null);
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [importPath, setImportPath] = useState("");
  const [exportPath, setExportPath] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { mode: themeMode, active: themeActive, setMode: setThemeMode } =
    useTheme();

  const refresh = useCallback(async () => {
    try {
      const [d, c, b] = await Promise.all([
        ipc.doctor(),
        ipc.configGet(),
        ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
      ]);
      setConfigPath(d.config_path ?? null);
      setIsDefault(d.is_default_browser);
      setDoc(c);
      setBrowsers(b);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh, configEpoch]);

  const updateDefaultTarget = async (next: BrowserTarget) => {
    if (!doc) return;
    setError(null);
    try {
      await ipc.configReplace({ ...doc, default_target: next });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const setAsDefault = async () => {
    setError(null);
    setMessage(null);
    try {
      const outcome: SetDefaultOutcome = await ipc.requestSetDefaultBrowser();
      if (outcome.kind === "done") {
        setMessage("Set successfully. macOS may show a confirmation dialog.");
      } else if (outcome.kind === "user-consent-required") {
        setMessage(
          outcome.instructions_url
            ? `Open ${outcome.instructions_url} to finish setting LinkPilot as default.`
            : "Please finish the default-browser switch in System Settings.",
        );
      } else {
        setMessage("This platform doesn't expose a programmatic 'set default'.");
      }
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const toggleLaunchAtLogin = async () => {
    if (!doc) return;
    const next: ConfigDocument = {
      ...doc,
      settings: {
        ...doc.settings,
        launch_at_login: !doc.settings.launch_at_login,
      },
    };
    try {
      await ipc.configReplace(next);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const doImport = async () => {
    setError(null);
    setMessage(null);
    try {
      await ipc.importConfig(importPath);
      setMessage(`Imported ${importPath}`);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const doExport = async () => {
    setError(null);
    setMessage(null);
    try {
      await ipc.exportConfig(exportPath);
      setMessage(`Exported to ${exportPath}`);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <>
      <h2>Settings</h2>
      <p className="subtitle">Default browser, autostart, and config IO.</p>

      <div className="card">
        <h3>Default browser</h3>
        <div className="row">
          <span className="grow">
            LinkPilot is currently the system default browser:
          </span>
          <span className={`tag ${isDefault ? "ok" : "danger"}`}>
            {isDefault === null ? "…" : isDefault ? "yes" : "no"}
          </span>
        </div>
        <div className="row">
          <span className="grow muted">
            macOS will prompt to confirm. On Windows you'll be sent to the
            Settings → Default apps page.
          </span>
          <button className="primary" onClick={setAsDefault}>
            Set LinkPilot as default
          </button>
        </div>
      </div>

      <div className="card">
        <h3>Default target</h3>
        <p className="muted" style={{ margin: "0 0 12px" }}>
          Where to open links when <strong>no rule matches</strong>. Changing
          this rewrites the config file.
        </p>
        {doc ? (
          <div className="row">
            <TargetEditor
              value={doc.default_target}
              browsers={browsers}
              onChange={updateDefaultTarget}
            />
          </div>
        ) : (
          <div className="muted">Loading…</div>
        )}
      </div>

      <div className="card">
        <h3>General</h3>
        <div className="row">
          <span className="grow">Launch at login</span>
          <input
            type="checkbox"
            checked={doc?.settings.launch_at_login ?? false}
            onChange={toggleLaunchAtLogin}
            style={{ width: "auto" }}
          />
        </div>
        <div className="row">
          <span className="grow">Config file</span>
          <span className="mono muted">{configPath ?? "…"}</span>
        </div>
      </div>

      <div className="card">
        <h3>Appearance</h3>
        <div className="row">
          <span className="grow">
            Theme
            {themeMode === "system" && (
              <span className="muted">
                {" "}
                — currently <span className="mono">{themeActive}</span>
              </span>
            )}
          </span>
          <select
            value={themeMode}
            onChange={(e) => setThemeMode(e.target.value as ThemeMode)}
            style={{ width: 160 }}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
      </div>

      <div className="card">
        <h3>Import / Export</h3>
        <div className="row">
          <input
            value={importPath}
            placeholder="/absolute/path/to/some.json"
            onChange={(e) => setImportPath(e.target.value)}
          />
          <button onClick={doImport} disabled={!importPath}>
            Import
          </button>
        </div>
        <div className="row">
          <input
            value={exportPath}
            placeholder="/absolute/path/to/save.json"
            onChange={(e) => setExportPath(e.target.value)}
          />
          <button onClick={doExport} disabled={!exportPath}>
            Export
          </button>
        </div>
      </div>

      {message && (
        <div className="card">
          <span className="tag ok">info</span>
          <span className="muted"> {message}</span>
        </div>
      )}
      {error && (
        <div className="card">
          <span className="tag danger">error</span>
          <span className="muted"> {error}</span>
        </div>
      )}
    </>
  );
}
