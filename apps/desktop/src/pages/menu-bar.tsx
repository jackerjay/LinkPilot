import { useCallback, useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import {
  AlertTriangle,
  Bolt,
  FileText,
  Globe,
  Layout,
  MousePointerClick,
} from "lucide-react";
import { ActivityCard } from "@/components/ActivityCard";
import { AppIcon } from "@/components/AppIcon";
import { BrowserBadge } from "@/components/BrowserBadge";
import { EmptyState } from "@/components/EmptyState";
import { TargetEditor } from "@/components/TargetEditor";
import brandIcon from "@/assets/brand.png";
import { appPathFromExecutable } from "@/lib/browsers";
import { ipc, onRouteLogged } from "@/lib/ipc";
import type {
  BrowserTarget,
  ConfigDocument,
  DoctorReport,
  InstalledBrowser,
  RouteRecord,
  RoutingDecision,
} from "@/lib/types";

interface Props {
  configEpoch: number;
}

export function MenuBarPage({ configEpoch }: Props) {
  const { t } = useTranslation("menuBar");
  const [doctor, setDoctor] = useState<DoctorReport | null>(null);
  const [config, setConfig] = useState<ConfigDocument | null>(null);
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  // Full window the daemon keeps in memory — powers the activity card
  // and the "routes today" stat; the Recent list shows the first 5.
  const [history, setHistory] = useState<RouteRecord[]>([]);
  const [defaultTargetError, setDefaultTargetError] = useState<string | null>(
    null,
  );

  const refresh = useCallback(async () => {
    const [d, c, h, b] = await Promise.all([
      ipc.doctor(),
      ipc.configGet(),
      ipc.routeHistory(200),
      ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
    ]);
    setDoctor(d);
    setConfig(c);
    setHistory(h);
    setBrowsers(b);
  }, []);

  useEffect(() => {
    refresh().catch(console.error);
  }, [refresh, configEpoch]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((record) => {
      setHistory((prev) => [record, ...prev].slice(0, 200));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const recent = history.slice(0, 5);
  const startOfToday = new Date().setHours(0, 0, 0, 0);
  const routesToday = history.filter(
    (r) => r.timestamp_ms >= startOfToday,
  ).length;
  const enabledRules = config?.rules.filter((r) => r.enabled).length ?? 0;
  const totalRules = config?.rules.length ?? 0;
  const disabledCount = totalRules - enabledRules;
  const daemonVersion = doctor?.daemon_version?.replace(
    /^linkpilot-daemon\s+/,
    "v",
  );
  const defaultTargetAvailable =
    !!config && browsers.some((b) => b.id === config.default_target.browser);
  const needsDefaultTarget =
    !!config &&
    (config.default_target.browser === "system" || !defaultTargetAvailable);
  const defaultTargetValue: BrowserTarget =
    config && defaultTargetAvailable
      ? config.default_target
      : { browser: "", profile: null, incognito: false, new_window: false };

  const updateDefaultTarget = async (next: BrowserTarget) => {
    if (!config || !next.browser) return;
    setDefaultTargetError(null);
    try {
      await ipc.configReplace({ ...config, default_target: next });
      await refresh();
    } catch (err) {
      setDefaultTargetError(String(err));
    }
  };

  if (needsDefaultTarget && config) {
    return (
      <SetupHero
        config={config}
        browsers={browsers}
        defaultTargetValue={defaultTargetValue}
        error={defaultTargetError}
        onPick={(next) => void updateDefaultTarget(next)}
      />
    );
  }

  return (
    <div>
      <h2 className="mac-h2">{t("title")}</h2>
      <p className="mac-subtitle">{t("subtitle")}</p>

      {/* At-a-glance stats */}
      <div className="mac-stat-grid">
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.routesToday")}</div>
          <div className="mac-stat-value">{routesToday}</div>
          <div className="mac-stat-trend">
            {routesToday > 0 ? t("stats.live") : t("stats.awaiting")}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.activeRules")}</div>
          <div className="mac-stat-value">
            {enabledRules}
            <span
              style={{
                color: "var(--mac-fg-muted)",
                fontSize: 15,
                fontWeight: 500,
              }}
            >
              {" "}
              / {totalRules}
            </span>
          </div>
          <div
            className="mac-stat-trend"
            style={{
              color:
                disabledCount > 0 ? "var(--mac-fg-muted)" : "var(--mac-ok)",
            }}
          >
            {disabledCount > 0
              ? t("stats.disabled", { count: disabledCount })
              : t("stats.allEnabled")}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.browsers")}</div>
          <div className="mac-stat-value">
            {doctor?.installed_browser_count ?? 0}
          </div>
          <div className="mac-stat-trend">{t("stats.detected")}</div>
        </div>
      </div>

      {/* 24h sparkline + per-browser distribution */}
      <ActivityCard history={history} />

      {/* Status card — System-Settings-shaped rows */}
      <div className="mac-card-title">{t("status.card")}</div>
      <div className="mac-card">
        <div className="mac-row">
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <Bolt size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">{t("status.daemon")}</span>
          <span
            className={`mac-dot ${doctor ? "ok" : "warn"}`}
            title={
              doctor
                ? t("status.daemonTitleRunning")
                : t("status.daemonTitleUnreachable")
            }
          />
          <span className="mac-row-value">
            {doctor
              ? t("status.daemonRunning")
              : t("status.daemonUnreachable")}
            {daemonVersion ? ` · ${daemonVersion}` : ""}
          </span>
        </div>
        <div className="mac-row">
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <Layout size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">
            {t("status.defaultBrowser")}
          </span>
          {doctor?.is_default_browser ? (
            <span
              style={{ display: "inline-flex", alignItems: "center", gap: 6 }}
            >
              <img
                src={brandIcon}
                width={18}
                height={18}
                alt=""
                style={{
                  borderRadius: 4,
                  flex: "0 0 18px",
                  boxShadow: "0 0 0 0.5px rgba(0,0,0,0.08)",
                }}
              />
              <span className="mac-row-value">LinkPilot</span>
            </span>
          ) : (
            <span className="mac-row-value">{t("status.defaultNotSet")}</span>
          )}
          <span
            className={`mac-tag ${doctor?.is_default_browser ? "ok" : "danger"}`}
          >
            {doctor?.is_default_browser
              ? t("status.tagActive")
              : t("status.tagNotSet")}
          </span>
        </div>
        <div className="mac-row">
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <Globe size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">
            {t("status.browsersDetected")}
          </span>
          <span className="mac-row-value">
            {t("status.browsersCount", {
              count: doctor?.installed_browser_count ?? 0,
            })}
          </span>
        </div>
        <div className="mac-row">
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <FileText size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">{t("status.configFile")}</span>
          <span
            className="select-text mac-mono mac-muted"
            style={{ fontSize: 11 }}
            title={doctor?.config_path ?? undefined}
          >
            {doctor?.config_path ?? "…"}
          </span>
        </div>
      </div>

      {/* Recent routes */}
      <div className="mac-card-title">{t("recent.card")}</div>
      <div className="mac-card">
        {recent.length === 0 ? (
          <EmptyState
            icon={MousePointerClick}
            title={t("recent.emptyTitle")}
            hint={
              <Trans
                i18nKey="recent.emptyHint"
                ns="menuBar"
                components={{
                  code: (
                    <span className="mac-mono" style={{ margin: "0 2px" }} />
                  ),
                }}
              />
            }
          />
        ) : (
          recent.map((r, i) => <RouteRow key={i} record={r} />)
        )}
      </div>
    </div>
  );
}

function RouteRow({ record }: { record: RouteRecord }) {
  const { t } = useTranslation("menuBar");
  return (
    <div className="mac-row clickable">
      <span
        className="mac-muted"
        style={{
          width: 70,
          fontSize: 12,
          fontVariantNumeric: "tabular-nums",
          flexShrink: 0,
        }}
      >
        {formatTimeAgo(t, record.timestamp_ms)}
      </span>
      <span
        className="grow mac-mono"
        style={{
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {record.context.url}
      </span>
      <DecisionPill decision={record.decision} />
    </div>
  );
}

export function DecisionPill({ decision }: { decision: RoutingDecision }) {
  const { t } = useTranslation("menuBar");
  if (decision.action === "open") {
    return (
      <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
        <BrowserBadge
          browserId={decision.target.browser}
          profile={decision.target.profile}
          className="text-xs"
        />
      </span>
    );
  }
  if (decision.action === "allow") {
    return <span className="mac-tag neutral">{t("decision.allow")}</span>;
  }
  if (decision.action === "block") {
    return <span className="mac-tag danger">{t("decision.block")}</span>;
  }
  return <span className="mac-tag warn">{t("decision.ask")}</span>;
}

// Re-export under the old name for components that still import
// `DecisionLine` from this module. New code should use `DecisionPill`.
export const DecisionLine = DecisionPill;

interface SetupHeroProps {
  config: ConfigDocument;
  browsers: InstalledBrowser[];
  defaultTargetValue: BrowserTarget;
  error: string | null;
  onPick: (next: BrowserTarget) => void;
}

function SetupHero({
  config,
  browsers,
  defaultTargetValue,
  error,
  onPick,
}: SetupHeroProps) {
  const { t } = useTranslation("menuBar");
  const hasBrowsers = browsers.length > 0;
  const currentMissing =
    config.default_target.browser !== "system" &&
    !browsers.some((b) => b.id === config.default_target.browser);

  return (
    <div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          marginBottom: 14,
        }}
      >
        <img
          src={brandIcon}
          width={40}
          height={40}
          alt="LinkPilot"
          style={{
            borderRadius: 9,
            flex: "0 0 40px",
            boxShadow: "0 0 0 0.5px rgba(0,0,0,0.08)",
          }}
        />
        <div>
          <h2 className="mac-h2" style={{ margin: 0 }}>
            {t("setupDefaultTarget.heroTitle")}
          </h2>
          <p className="mac-subtitle" style={{ margin: "2px 0 0" }}>
            {t("setupDefaultTarget.heroSubtitle")}
          </p>
        </div>
      </div>

      <div className="mac-card" style={{ padding: "14px 16px" }}>
        {currentMissing && (
          <div
            style={{
              display: "flex",
              alignItems: "flex-start",
              gap: 8,
              padding: "10px 12px",
              marginBottom: 12,
              borderRadius: 8,
              background:
                "color-mix(in srgb, var(--mac-warn) 14%, transparent)",
              color: "var(--mac-warn)",
              fontSize: 12,
            }}
          >
            <AlertTriangle size={14} strokeWidth={2} style={{ marginTop: 1 }} />
            <span>
              {t("setupDefaultTarget.currentMissing", {
                browser: config.default_target.browser,
              })}
            </span>
          </div>
        )}

        {hasBrowsers ? (
          <>
            <div
              className="mac-row-label"
              style={{ marginBottom: 8, fontSize: 12 }}
            >
              {t("setupDefaultTarget.quickPickLabel")}
            </div>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
                gap: 8,
                marginBottom: 16,
              }}
            >
              {browsers.map((b) => (
                <QuickPickTile
                  key={b.id}
                  browser={b}
                  onPick={() =>
                    onPick({
                      browser: b.id,
                      profile: null,
                      incognito: false,
                      new_window: false,
                    })
                  }
                />
              ))}
            </div>

            <div
              className="mac-row-label"
              style={{ marginBottom: 8, fontSize: 12 }}
            >
              {t("setupDefaultTarget.advancedLabel")}
            </div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                flexWrap: "wrap",
              }}
            >
              <TargetEditor
                value={defaultTargetValue}
                browsers={browsers}
                onChange={onPick}
              />
            </div>
          </>
        ) : (
          <div
            className="mac-muted"
            style={{ fontSize: 12, padding: "8px 0" }}
          >
            {t("setupDefaultTarget.noBrowsers")}
          </div>
        )}

        {error && (
          <div
            style={{
              color: "var(--mac-danger)",
              fontSize: 11.5,
              marginTop: 10,
            }}
          >
            {error}
          </div>
        )}

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginTop: 14,
            paddingTop: 10,
            borderTop: "0.5px solid var(--mac-border-soft)",
            fontSize: 11.5,
          }}
        >
          <span className="grow mac-muted">
            {t("setupDefaultTarget.askFallback")}
          </span>
          <span className="mac-tag warn">{t("decision.ask")}</span>
        </div>
      </div>
    </div>
  );
}

interface QuickPickTileProps {
  browser: InstalledBrowser;
  onPick: () => void;
}

function QuickPickTile({ browser, onPick }: QuickPickTileProps) {
  return (
    <button
      type="button"
      onClick={onPick}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        padding: "8px 10px",
        borderRadius: 8,
        border: "0.5px solid var(--mac-border-soft)",
        background: "var(--mac-card-fill)",
        color: "inherit",
        cursor: "pointer",
        textAlign: "left",
        fontSize: 12.5,
        width: "100%",
        transition: "background 120ms ease",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background =
          "color-mix(in srgb, var(--mac-card-fill) 88%, var(--mac-accent) 12%)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "var(--mac-card-fill)";
      }}
    >
      <AppIcon
        bundleId={browser.platform_app_id ?? undefined}
        appPath={appPathFromExecutable(browser.executable)}
        size={20}
        alt={browser.display_name}
      />
      <span
        style={{
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {browser.display_name}
      </span>
    </button>
  );
}

function formatTimeAgo(
  t: (key: string, opts?: Record<string, unknown>) => string,
  ms: number,
): string {
  const s = Math.round((Date.now() - ms) / 1000);
  if (s < 60) return t("timeAgo.seconds", { n: s });
  if (s < 3600) return t("timeAgo.minutes", { n: Math.round(s / 60) });
  if (s < 86400) return t("timeAgo.hours", { n: Math.round(s / 3600) });
  return t("timeAgo.days", { n: Math.round(s / 86400) });
}
