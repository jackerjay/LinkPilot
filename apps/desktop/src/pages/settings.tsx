import { useCallback, useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
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
import { PickerStyleChooser } from "@/picker/halo/PickerStyleChooser";
import { ProfileOrderEditor } from "@/picker/halo/ProfileOrderEditor";
import { ipc } from "@/lib/ipc";
import { useTheme, type ThemeMode } from "@/lib/theme";
import type { UpdateCheckState } from "@/lib/update";
import type {
  BrowserTarget,
  ConfigDocument,
  InstalledBrowser,
  LanguagePref,
  PickerStyle,
  SetDefaultOutcome,
} from "@/lib/types";
import type { CliInstallStatus, DaemonServiceStatus } from "@/lib/ipc";
import { LANGUAGE_LABELS, SUPPORTED_LANGUAGES } from "@/i18n/languages";
import brandIcon from "@/assets/brand.png";

interface Props {
  configEpoch: number;
  updateCheck: UpdateCheckState;
  onCheckForUpdates: () => Promise<void>;
}

export function SettingsPage({
  configEpoch,
  updateCheck,
  onCheckForUpdates,
}: Props) {
  const { t } = useTranslation("settings");
  const { t: tSuggestions } = useTranslation("suggestions");
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
        setMessage(t("defaultBrowser.successDefault"));
      } else if (outcome.kind === "user-consent-required") {
        setMessage(
          outcome.instructions_url
            ? t("defaultBrowser.successOtherWithUrl", {
                url: outcome.instructions_url,
              })
            : t("defaultBrowser.successOther"),
        );
      } else {
        setMessage(t("defaultBrowser.unsupported"));
      }
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const changePickerStyle = async (next: PickerStyle) => {
    try {
      await ipc.setPickerStyle(next);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const changeLanguage = async (next: LanguagePref) => {
    try {
      await ipc.setLanguage(next);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const defaultTargetAvailable =
    !!doc && browsers.some((b) => b.id === doc.default_target.browser);
  const defaultTargetValue: BrowserTarget =
    doc && defaultTargetAvailable
      ? doc.default_target
      : { browser: "", profile: null, incognito: false, new_window: false };

  const toggleLaunchAtLogin = async (next: boolean) => {
    if (!doc) return;
    try {
      await ipc.setLaunchAtLogin(next);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const toggleAutoCheckUpdates = async (next: boolean) => {
    if (!doc) return;
    try {
      await ipc.configReplace({
        ...doc,
        settings: { ...doc.settings, auto_check_updates: next },
      });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const toggleBehaviorLog = async (next: boolean) => {
    if (!doc) return;
    try {
      await ipc.configReplace({
        ...doc,
        settings: { ...doc.settings, behavior_log_enabled: next },
      });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const setBehaviorRetention = async (value: string) => {
    if (!doc) return;
    const next = value === "forever" ? null : Number(value);
    try {
      await ipc.configReplace({
        ...doc,
        settings: { ...doc.settings, behavior_log_retention_days: next },
      });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const clearBehaviorLog = async () => {
    if (!doc) return;
    if (!window.confirm(tSuggestions("settings.clearConfirm"))) return;
    try {
      await ipc.observationsClear();
      setMessage(tSuggestions("settings.clearedToast"));
    } catch (err) {
      setError(String(err));
    }
  };

  const doImport = async () => {
    setError(null);
    setMessage(null);
    try {
      await ipc.importConfig(importPath);
      setMessage(t("io.successImported", { path: importPath }));
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
      setMessage(t("io.successExported", { path: exportPath }));
    } catch (err) {
      setError(String(err));
    }
  };

  const installCli = async () => {
    setError(null);
    setMessage(null);
    try {
      const installed = await ipc.cliInstallToPath();
      setMessage(t("cli.successInstalled", { path: installed }));
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
      setMessage(t("daemon.successInstalled"));
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
      setMessage(t("daemon.successUninstalled"));
    } catch (err) {
      setError(String(err));
    }
  };

  const openReleasePage = async () => {
    const releaseUrl =
      updateCheck.status === "downloaded"
        ? updateCheck.result.releaseUrl
        : updateCheck.status === "error"
          ? updateCheck.result?.releaseUrl
            : null;
    if (!releaseUrl) return;
    setError(null);
    try {
      await openUrl(releaseUrl);
    } catch (err) {
      setError(String(err));
    }
  };

  const openDownloadedInstaller = async () => {
    if (updateCheck.status !== "downloaded") return;
    setError(null);
    try {
      await openPath(updateCheck.download.path);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div>
      <h2 className="mac-h2">{t("title")}</h2>
      <p className="mac-subtitle">{t("subtitle")}</p>

      <div className="mac-card-title">{t("defaultBrowser.card")}</div>
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
                ? t("defaultBrowser.isDefault")
                : t("defaultBrowser.isNotDefault")}
            </div>
            <div className="mac-muted" style={{ fontSize: 11.5 }}>
              {t("defaultBrowser.summary")}
            </div>
          </div>
          <span className={`mac-tag ${isDefault ? "ok" : "danger"}`}>
            {isDefault === null
              ? t("defaultBrowser.tagLoading")
              : isDefault
                ? t("defaultBrowser.tagActive")
                : t("defaultBrowser.tagNotSet")}
          </span>
        </div>
        {!isDefault && (
          <div className="mac-row">
            <span className="grow mac-muted" style={{ fontSize: 12 }}>
              {t("defaultBrowser.hint")}
            </span>
            <button
              type="button"
              className="mac-tbtn primary"
              onClick={setAsDefault}
            >
              {t("defaultBrowser.change")}
            </button>
          </div>
        )}
      </div>

      <div className="mac-card-title">{t("defaultTarget.card")}</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">{t("defaultTarget.label")}</div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              <Trans
                i18nKey="defaultTarget.description"
                ns="settings"
                components={{ strong: <strong /> }}
              />
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
                value={defaultTargetValue}
                browsers={browsers}
                onChange={updateDefaultTarget}
              />
            </div>
          ) : (
            <span className="mac-muted">{t("common:status.loading", "Loading…")}</span>
          )}
        </div>
      </div>

      <div className="mac-card-title">{t("appearance.card")}</div>
      <div className="mac-card">
        <div className="mac-row">
          <span className="grow mac-row-label">
            {t("appearance.themeLabel")}
            {themeMode === "system" && (
              <span
                className="mac-muted"
                style={{ marginLeft: 6, fontSize: 11.5 }}
              >
                {" "}
                <Trans
                  i18nKey="appearance.themeCurrent"
                  ns="settings"
                  values={{ value: themeActive }}
                  components={{ code: <span className="mac-mono" /> }}
                />
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
              <SelectItem value="system">{t("appearance.themeSystem")}</SelectItem>
              <SelectItem value="light">{t("appearance.themeLight")}</SelectItem>
              <SelectItem value="dark">{t("appearance.themeDark")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="mac-row">
          <span className="grow">
            <span className="mac-row-label">{t("appearance.languageLabel")}</span>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {t("appearance.languageDescription")}
            </div>
          </span>
          <Select
            value={doc?.settings.language ?? "system"}
            onValueChange={(v) => void changeLanguage(v as LanguagePref)}
          >
            <SelectTrigger className="w-[160px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="system">
                {t("appearance.languageSystem")}
              </SelectItem>
              {SUPPORTED_LANGUAGES.map((code) => (
                <SelectItem key={code} value={code}>
                  {LANGUAGE_LABELS[code]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div
          className="mac-row"
          style={{ alignItems: "flex-start", flexDirection: "column", gap: 4 }}
        >
          <div style={{ width: "100%" }}>
            <div className="mac-row-label">{t("appearance.pickerStyleLabel")}</div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {t("appearance.pickerStyleDescription")}
            </div>
          </div>
          <PickerStyleChooser
            value={doc?.settings.picker_style ?? "frosted"}
            onChange={(v) => changePickerStyle(v)}
          />
        </div>
        <div
          className="mac-row"
          style={{ alignItems: "flex-start", flexDirection: "column", gap: 4 }}
        >
          <div style={{ width: "100%" }}>
            <div className="mac-row-label">{t("appearance.profileOrderLabel")}</div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {t("appearance.profileOrderDescription")}
            </div>
          </div>
          <ProfileOrderEditor
            doc={doc}
            pickerStyle={doc?.settings.picker_style ?? "frosted"}
            onConfigChanged={refresh}
          />
        </div>
      </div>

      <div className="mac-card-title">{t("general.card")}</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">{t("general.updatesLabel")}</div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {describeUpdateCheck(t, updateCheck)}
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button
              type="button"
              className="mac-tbtn"
              onClick={() => void onCheckForUpdates()}
              disabled={
                updateCheck.status === "checking" ||
                updateCheck.status === "downloading"
              }
            >
              {updateCheck.status === "checking"
                ? t("general.checking")
                : updateCheck.status === "downloading"
                  ? t("general.downloading")
                  : t("general.checkNow")}
            </button>
            {updateCheck.status === "downloaded" && (
              <button
                type="button"
                className="mac-tbtn primary"
                onClick={() => void openDownloadedInstaller()}
              >
                {t("general.openInstaller")}
              </button>
            )}
            {updateCheck.status === "error" && updateCheck.result && (
              <button
                type="button"
                className="mac-tbtn"
                onClick={() => void openReleasePage()}
              >
                {t("general.openRelease")}
              </button>
            )}
          </div>
        </div>
        <div className="mac-row">
          <span className="grow mac-row-label">{t("general.autoUpdate")}</span>
          <button
            type="button"
            className={`mac-switch accent ${doc?.settings.auto_check_updates ? "on" : ""}`}
            aria-pressed={!!doc?.settings.auto_check_updates}
            onClick={() =>
              toggleAutoCheckUpdates(!doc?.settings.auto_check_updates)
            }
          />
        </div>
        <div className="mac-row">
          <span className="grow mac-row-label">{t("general.launchAtLogin")}</span>
          <button
            type="button"
            className={`mac-switch accent ${doc?.settings.launch_at_login ? "on" : ""}`}
            aria-pressed={!!doc?.settings.launch_at_login}
            onClick={() => toggleLaunchAtLogin(!doc?.settings.launch_at_login)}
          />
        </div>
        <div className="mac-row">
          <span className="grow mac-row-label">{t("general.configFile")}</span>
          <span
            className="select-text mac-mono mac-muted"
            style={{ fontSize: 11 }}
            title={configPath ?? undefined}
          >
            {configPath ?? "…"}
          </span>
        </div>
      </div>

      <div className="mac-card-title">{tSuggestions("settings.title")}</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">
              {tSuggestions("settings.enableLabel")}
            </div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {tSuggestions("settings.hint")}
            </div>
          </div>
          <button
            type="button"
            className={`mac-switch accent ${doc?.settings.behavior_log_enabled ? "on" : ""}`}
            aria-pressed={!!doc?.settings.behavior_log_enabled}
            onClick={() =>
              toggleBehaviorLog(!doc?.settings.behavior_log_enabled)
            }
          />
        </div>
        <div className="mac-row">
          <span className="grow mac-row-label">
            {tSuggestions("settings.retentionLabel")}
          </span>
          <Select
            value={
              doc?.settings.behavior_log_retention_days == null
                ? "forever"
                : String(doc.settings.behavior_log_retention_days)
            }
            onValueChange={(v) => void setBehaviorRetention(v)}
            disabled={!doc?.settings.behavior_log_enabled}
          >
            <SelectTrigger style={{ minWidth: 140 }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="forever">
                {tSuggestions("settings.retention.forever")}
              </SelectItem>
              <SelectItem value="30">
                {tSuggestions("settings.retention.30")}
              </SelectItem>
              <SelectItem value="90">
                {tSuggestions("settings.retention.90")}
              </SelectItem>
              <SelectItem value="180">
                {tSuggestions("settings.retention.180")}
              </SelectItem>
              <SelectItem value="365">
                {tSuggestions("settings.retention.365")}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="mac-row">
          <span className="grow mac-row-label">
            {tSuggestions("settings.clearLabel")}
          </span>
          <button
            type="button"
            className="mac-tbtn"
            onClick={() => void clearBehaviorLog()}
          >
            {tSuggestions("settings.clearLabel")}
          </button>
        </div>
      </div>

      <div className="mac-card-title">{t("daemon.card")}</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">
              <span className="mac-mono">linkpilot-daemon</span>
              {daemonStatus?.loaded && (
                <span className="mac-tag ok" style={{ marginLeft: 8 }}>
                  {daemonStatus.pid
                    ? t("daemon.tagRunningWithPid", { pid: daemonStatus.pid })
                    : t("daemon.tagRunning")}
                </span>
              )}
              {daemonStatus?.plist_exists && !daemonStatus.loaded && (
                <span className="mac-tag" style={{ marginLeft: 8 }}>
                  {t("daemon.tagInstalledNotLoaded")}
                </span>
              )}
            </div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {daemonStatus?.bundled_path
                ? daemonStatus.plist_exists
                  ? t("daemon.descLoaded", { mode: daemonStatus.gui_mode })
                  : t("daemon.descInstall")
                : t("daemon.descDev")}
            </div>
          </div>
          {daemonStatus?.plist_exists ? (
            <button
              type="button"
              className="mac-tbtn"
              onClick={uninstallDaemonService}
            >
              {t("daemon.uninstall")}
            </button>
          ) : (
            <button
              type="button"
              className="mac-tbtn primary"
              onClick={installDaemonService}
              disabled={!daemonStatus?.bundled_path}
            >
              {t("daemon.install")}
            </button>
          )}
        </div>
        {daemonStatus?.bundled_path && (
          <div className="mac-row">
            <span className="grow mac-row-label">{t("daemon.bundledAt")}</span>
            <span
              className="select-text mac-mono mac-muted"
              style={{ fontSize: 11 }}
            >
              {daemonStatus.bundled_path}
            </span>
          </div>
        )}
      </div>

      <div className="mac-card-title">{t("cli.card")}</div>
      <div className="mac-card">
        <div className="mac-row" style={{ alignItems: "flex-start" }}>
          <div className="grow">
            <div className="mac-row-label">
              <Trans
                i18nKey="cli.label"
                ns="settings"
                components={{ code: <span className="mac-mono" /> }}
              />
              {cliStatus?.already_installed && (
                <span className="mac-tag ok" style={{ marginLeft: 8 }}>
                  {t("cli.tagInstalled")}
                </span>
              )}
            </div>
            <div
              className="mac-muted"
              style={{ fontSize: 11.5, marginTop: 2 }}
            >
              {cliStatus?.bundled_path
                ? t("cli.descBundled")
                : t("cli.descDev")}
            </div>
          </div>
          <button
            type="button"
            className="mac-tbtn primary"
            onClick={installCli}
            disabled={!cliStatus?.bundled_path}
          >
            {cliStatus?.already_installed
              ? t("cli.reinstall")
              : t("cli.install")}
          </button>
        </div>
        {cliStatus?.bundled_path && (
          <div className="mac-row">
            <span className="grow mac-row-label">{t("cli.bundledAt")}</span>
            <span
              className="select-text mac-mono mac-muted"
              style={{ fontSize: 11 }}
            >
              {cliStatus.bundled_path}
            </span>
          </div>
        )}
      </div>

      <div className="mac-card-title">{t("io.card")}</div>
      <div className="mac-card mac-card-pad" style={{ display: "grid", gap: 12 }}>
        <div>
          <div className="mac-muted" style={{ fontSize: 11, marginBottom: 4 }}>
            {t("io.importLabel")}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Input
              value={importPath}
              placeholder={t("io.importPlaceholder")}
              onChange={(e) => setImportPath(e.target.value)}
            />
            <Button
              variant="outline"
              onClick={doImport}
              disabled={!importPath}
            >
              {t("io.importButton")}
            </Button>
          </div>
        </div>
        <div>
          <div className="mac-muted" style={{ fontSize: 11, marginBottom: 4 }}>
            {t("io.exportLabel")}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Input
              value={exportPath}
              placeholder={t("io.exportPlaceholder")}
              onChange={(e) => setExportPath(e.target.value)}
            />
            <Button
              variant="outline"
              onClick={doExport}
              disabled={!exportPath}
            >
              {t("io.exportButton")}
            </Button>
          </div>
        </div>
      </div>

      {message && (
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag ok">{t("feedback.infoTag")}</span>
            <span className="grow mac-muted">{message}</span>
          </div>
        </div>
      )}
      {error && (
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag danger">{t("feedback.errorTag")}</span>
            <span className="grow mac-muted">{error}</span>
          </div>
        </div>
      )}
    </div>
  );
}

function describeUpdateCheck(
  t: TFunction<"settings">,
  state: UpdateCheckState,
): string {
  switch (state.status) {
    case "idle":
      return t("updateState.idle");
    case "checking":
      return t("updateState.checking");
    case "downloading":
      return t("updateState.downloading", {
        version: displayVersion(state.result.latestVersion),
        asset: state.result.asset.name,
      });
    case "downloaded":
      return t("updateState.downloaded", {
        version: displayVersion(state.result.latestVersion),
      });
    case "up-to-date":
      return t("updateState.upToDate", {
        version: displayVersion(state.result.currentVersion),
      });
    case "error":
      if (state.result?.available) {
        return t("updateState.errorWithVersion", {
          version: displayVersion(state.result.latestVersion),
          error: state.error,
        });
      }
      return t("updateState.errorPlain", { error: state.error });
  }
}

function displayVersion(version: string): string {
  return version.startsWith("v") || version.startsWith("V")
    ? version
    : `v${version}`;
}
