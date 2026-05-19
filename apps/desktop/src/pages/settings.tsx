import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TargetEditor } from "@/components/TargetEditor";
import { ipc } from "@/lib/ipc";
import { useTheme, type ThemeMode } from "@/lib/theme";
import type {
  BrowserTarget,
  ConfigDocument,
  InstalledBrowser,
  SetDefaultOutcome,
} from "@/lib/types";
import type { CliInstallStatus, DaemonServiceStatus } from "@/lib/ipc";
import brandIcon from "@/assets/brand.png";

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
  const [cliStatus, setCliStatus] = useState<CliInstallStatus | null>(null);
  const [daemonStatus, setDaemonStatus] = useState<DaemonServiceStatus | null>(
    null,
  );
  const { mode: themeMode, active: themeActive, setMode: setThemeMode } =
    useTheme();

  const refresh = useCallback(async () => {
    try {
      const [d, c, b, cli, daemon] = await Promise.all([
        ipc.doctor(),
        ipc.configGet(),
        ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
        ipc.cliInstallStatus().catch(() => null),
        ipc.daemonServiceStatus().catch(() => null),
      ]);
      setConfigPath(d.config_path ?? null);
      setIsDefault(d.is_default_browser);
      setDoc(c);
      setBrowsers(b);
      setCliStatus(cli);
      setDaemonStatus(daemon);
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

  const toggleLaunchAtLogin = async (next: boolean) => {
    if (!doc) return;
    try {
      await ipc.configReplace({
        ...doc,
        settings: { ...doc.settings, launch_at_login: next },
      });
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

  const installCli = async () => {
    setError(null);
    setMessage(null);
    try {
      const installed = await ipc.cliInstallToPath();
      setMessage(
        `Installed: ${installed}. Add ~/.local/bin to your PATH if it isn't already.`,
      );
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const installDaemonService = async () => {
    setError(null);
    setMessage(null);
    try {
      const next = await ipc.daemonServiceInstall();
      setDaemonStatus(next);
      setMessage(
        "Background service installed. The daemon will start now and on every login.",
      );
    } catch (err) {
      setError(String(err));
    }
  };

  const uninstallDaemonService = async () => {
    setError(null);
    setMessage(null);
    try {
      const next = await ipc.daemonServiceUninstall();
      setDaemonStatus(next);
      setMessage(
        "Background service removed. The daemon won't auto-start anymore.",
      );
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div>
      <h2 className="mac-h2">Settings</h2>
      <p className="mac-subtitle">
        Default browser, autostart, appearance, and config IO.
      </p>

      <div className="mac-card-title">Default browser</div>
      <div className="mac-card">
        <div className="mac-row">
          <img
            src={brandIcon}
            width={22}
            height={22}
            alt=""
            style={{
              borderRadius: 5,
              flex: "0 0 22px",
              boxShadow: "0 0 0 0.5px rgba(0,0,0,0.08)",
            }}
          />
          <div className="grow">
            <div>
              {isDefault
                ? "LinkPilot is the system default browser"
                : "LinkPilot is not the default browser"}
            </div>
            <div className="mac-muted" style={{ fontSize: 11.5 }}>
              All URLs route through LinkPilot's rules engine.
            </div>
          </div>
          <span className={`mac-tag ${isDefault ? "ok" : "danger"}`}>
            {isDefault === null ? "…" : isDefault ? "active" : "not set"}
          </span>
        </div>
        {!isDefault && (
          <div className="mac-row">
            <span className="grow mac-muted" style={{ fontSize: 12 }}>
              macOS will prompt to confirm. On Windows you'll be sent to
              Settings → Default apps.
            </span>
            <button
              type="button"
              className="mac-tbtn primary"
              onClick={setAsDefault}
            >
              Change…
            </button>
          </div>
        )}
      </div>

      <div className="mac-card-title">Default target</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">Default target</div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              Where to open links when <strong>no rule matches</strong>.
              Changing this rewrites the config file.
            </div>
          </div>
          {doc ? (
            // TargetEditor returns a fragment of (browser select, profile
            // select, incognito checkbox). Without a flex container they
            // stack vertically because shadcn's SelectTrigger is a
            // block-level button. Wrap them so they sit on one row at
            // the right of the description, matching System Settings.
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                flexWrap: "wrap",
                justifyContent: "flex-end",
              }}
            >
              <TargetEditor
                value={doc.default_target}
                browsers={browsers}
                onChange={updateDefaultTarget}
              />
            </div>
          ) : (
            <span className="mac-muted">Loading…</span>
          )}
        </div>
      </div>

      <div className="mac-card-title">Appearance</div>
      <div className="mac-card">
        <div className="mac-row">
          <span className="grow mac-row-label">
            Theme
            {themeMode === "system" && (
              <span
                className="mac-muted"
                style={{ marginLeft: 6, fontSize: 11.5 }}
              >
                — currently <span className="mac-mono">{themeActive}</span>
              </span>
            )}
          </span>
          <Select
            value={themeMode}
            onValueChange={(v) => setThemeMode(v as ThemeMode)}
          >
            <SelectTrigger className="w-[140px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="system">System</SelectItem>
              <SelectItem value="light">Light</SelectItem>
              <SelectItem value="dark">Dark</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="mac-card-title">General</div>
      <div className="mac-card">
        <div className="mac-row">
          <span className="grow mac-row-label">Launch at login</span>
          <button
            type="button"
            className={`mac-switch accent ${doc?.settings.launch_at_login ? "on" : ""}`}
            aria-pressed={!!doc?.settings.launch_at_login}
            onClick={() => toggleLaunchAtLogin(!doc?.settings.launch_at_login)}
          />
        </div>
        <div className="mac-row">
          <span className="grow mac-row-label">Configuration file</span>
          <span
            className="select-text mac-mono mac-muted"
            style={{ fontSize: 11 }}
            title={configPath ?? undefined}
          >
            {configPath ?? "…"}
          </span>
        </div>
      </div>

      <div className="mac-card-title">Background service</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">
              <span className="mac-mono">linkpilot-daemon</span>
              {daemonStatus?.loaded && (
                <span className="mac-tag ok" style={{ marginLeft: 8 }}>
                  running
                  {daemonStatus.pid ? ` · pid ${daemonStatus.pid}` : ""}
                </span>
              )}
              {daemonStatus?.plist_exists && !daemonStatus.loaded && (
                <span className="mac-tag" style={{ marginLeft: 8 }}>
                  installed · not loaded
                </span>
              )}
            </div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {daemonStatus?.bundled_path
                ? daemonStatus.plist_exists
                  ? `LaunchAgent loads at every login. GUI runs in "${daemonStatus.gui_mode}" mode.`
                  : "Install the LaunchAgent so the router keeps working when LinkPilot.app is closed."
                : "No bundled daemon found — you're on a dev build. Releases ship the daemon embedded in the .app."}
            </div>
          </div>
          {daemonStatus?.plist_exists ? (
            <button
              type="button"
              className="mac-tbtn"
              onClick={uninstallDaemonService}
            >
              Uninstall
            </button>
          ) : (
            <button
              type="button"
              className="mac-tbtn primary"
              onClick={installDaemonService}
              disabled={!daemonStatus?.bundled_path}
            >
              Install background service
            </button>
          )}
        </div>
        {daemonStatus?.bundled_path && (
          <div className="mac-row">
            <span className="grow mac-row-label">Bundled at</span>
            <span
              className="select-text mac-mono mac-muted"
              style={{ fontSize: 11 }}
            >
              {daemonStatus.bundled_path}
            </span>
          </div>
        )}
      </div>

      <div className="mac-card-title">Command-line tool</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">
              <span className="mac-mono">lpt</span> CLI
              {cliStatus?.already_installed && (
                <span className="mac-tag ok" style={{ marginLeft: 8 }}>
                  installed
                </span>
              )}
            </div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {cliStatus?.bundled_path
                ? "The bundled binary lives inside this .app. Installing creates a symlink at ~/.local/bin/lpt so `lpt` works from any shell."
                : "No bundled `lpt` found — you're on a dev build. Releases ship the CLI embedded in the .app."}
            </div>
          </div>
          <button
            type="button"
            className="mac-tbtn primary"
            onClick={installCli}
            disabled={!cliStatus?.bundled_path}
          >
            {cliStatus?.already_installed
              ? "Reinstall"
              : "Install to ~/.local/bin"}
          </button>
        </div>
        {cliStatus?.bundled_path && (
          <div className="mac-row">
            <span className="grow mac-row-label">Bundled at</span>
            <span
              className="select-text mac-mono mac-muted"
              style={{ fontSize: 11 }}
            >
              {cliStatus.bundled_path}
            </span>
          </div>
        )}
      </div>

      <div className="mac-card-title">Import / Export</div>
      <div className="mac-card mac-card-pad" style={{ display: "grid", gap: 12 }}>
        <div>
          <div className="mac-muted" style={{ fontSize: 11, marginBottom: 4 }}>
            Import config from path
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Input
              value={importPath}
              placeholder="/absolute/path/to/some.json"
              onChange={(e) => setImportPath(e.target.value)}
            />
            <Button
              variant="outline"
              onClick={doImport}
              disabled={!importPath}
            >
              Import
            </Button>
          </div>
        </div>
        <div>
          <div className="mac-muted" style={{ fontSize: 11, marginBottom: 4 }}>
            Export config to path
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Input
              value={exportPath}
              placeholder="/absolute/path/to/save.json"
              onChange={(e) => setExportPath(e.target.value)}
            />
            <Button
              variant="outline"
              onClick={doExport}
              disabled={!exportPath}
            >
              Export
            </Button>
          </div>
        </div>
      </div>

      {message && (
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag ok">info</span>
            <span className="grow mac-muted">{message}</span>
          </div>
        </div>
      )}
      {error && (
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag danger">error</span>
            <span className="grow mac-muted">{error}</span>
          </div>
        </div>
      )}
    </div>
  );
}
