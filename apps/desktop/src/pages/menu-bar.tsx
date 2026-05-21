import { useCallback, useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { Bolt, FileText, Globe, Layout } from "lucide-react";
import { BrowserBadge } from "@/components/BrowserBadge";
import brandIcon from "@/assets/brand.png";
import { ipc, onRouteLogged } from "@/lib/ipc";
import type {
  ConfigDocument,
  DoctorReport,
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
  const [recent, setRecent] = useState<RouteRecord[]>([]);

  const refresh = useCallback(async () => {
    const [d, c, h] = await Promise.all([
      ipc.doctor(),
      ipc.configGet(),
      ipc.routeHistory(5),
    ]);
    setDoctor(d);
    setConfig(c);
    setRecent(h);
  }, []);

  useEffect(() => {
    refresh().catch(console.error);
  }, [refresh, configEpoch]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((record) => {
      setRecent((prev) => [record, ...prev].slice(0, 5));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const enabledRules = config?.rules.filter((r) => r.enabled).length ?? 0;
  const totalRules = config?.rules.length ?? 0;
  const disabledCount = totalRules - enabledRules;
  const daemonVersion = doctor?.daemon_version?.replace(
    /^linkpilot-daemon\s+/,
    "v",
  );

  return (
    <div>
      <h2 className="mac-h2">{t("title")}</h2>
      <p className="mac-subtitle">{t("subtitle")}</p>

      {/* At-a-glance stats */}
      <div className="mac-stat-grid">
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.routesToday")}</div>
          <div className="mac-stat-value">{recent.length}</div>
          <div className="mac-stat-trend">
            {recent.length > 0 ? t("stats.live") : t("stats.awaiting")}
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
          <div
            className="mac-row mac-muted"
            style={{ justifyContent: "center", padding: "24px 18px" }}
          >
            <Trans
              i18nKey="recent.empty"
              ns="menuBar"
              components={{
                code: <span className="mac-mono" style={{ margin: "0 4px" }} />,
              }}
            />
          </div>
        ) : (
          recent
            .slice(0, 5)
            .map((r, i) => <RouteRow key={i} record={r} />)
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

