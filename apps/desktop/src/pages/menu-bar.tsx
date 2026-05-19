import { useCallback, useEffect, useState } from "react";
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
      <h2 className="mac-h2">Overview</h2>
      <p className="mac-subtitle">
        Daemon status and the most recent routing decisions.
      </p>

      {/* At-a-glance stats */}
      <div className="mac-stat-grid">
        <div className="mac-stat">
          <div className="mac-stat-label">Routes today</div>
          <div className="mac-stat-value">{recent.length}</div>
          <div className="mac-stat-trend">
            {recent.length > 0 ? "Live" : "Awaiting clicks"}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">Active rules</div>
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
            {disabledCount > 0 ? `${disabledCount} disabled` : "All enabled"}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">Browsers</div>
          <div className="mac-stat-value">
            {doctor?.installed_browser_count ?? 0}
          </div>
          <div className="mac-stat-trend">Detected</div>
        </div>
      </div>

      {/* Status card — System-Settings-shaped rows */}
      <div className="mac-card-title">Status</div>
      <div className="mac-card">
        <div className="mac-row">
          {/* Leading icon stays neutral — the green status signal lives
              on the right-side dot now, so the icon doesn't double up
              as a status indicator. */}
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <Bolt size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">Daemon</span>
          <span
            className={`mac-dot ${doctor ? "ok" : "warn"}`}
            title={doctor ? "Daemon is running" : "Daemon is unreachable"}
          />
          <span className="mac-row-value">
            {doctor ? "running" : "unreachable"}
            {daemonVersion ? ` · ${daemonVersion}` : ""}
          </span>
        </div>
        <div className="mac-row">
          {/* Leading icon stays neutral — generic "default browser
              setting" affordance. The *identity* of the current default
              (its app icon) renders on the right next to the value. */}
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <Layout size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">System default browser</span>
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
            <span className="mac-row-value">Not set</span>
          )}
          <span
            className={`mac-tag ${doctor?.is_default_browser ? "ok" : "danger"}`}
          >
            {doctor?.is_default_browser ? "active" : "not set"}
          </span>
        </div>
        <div className="mac-row">
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <Globe size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">Browsers detected</span>
          <span className="mac-row-value">
            {doctor?.installed_browser_count ?? 0} apps
          </span>
        </div>
        <div className="mac-row">
          <span style={{ color: "var(--mac-fg-muted)" }}>
            <FileText size={15} strokeWidth={1.8} />
          </span>
          <span className="grow mac-row-label">Configuration</span>
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
      <div className="mac-card-title">Recent routes</div>
      <div className="mac-card">
        {recent.length === 0 ? (
          <div
            className="mac-row mac-muted"
            style={{ justifyContent: "center", padding: "24px 18px" }}
          >
            No routes yet. Try{" "}
            <span className="mac-mono" style={{ margin: "0 4px" }}>
              lpt open …
            </span>{" "}
            or click a link.
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
        {timeAgo(record.timestamp_ms)}
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
    return <span className="mac-tag neutral">allow</span>;
  }
  if (decision.action === "block") {
    return <span className="mac-tag danger">blocked</span>;
  }
  return <span className="mac-tag warn">ask</span>;
}

// Re-export under the old name for components that still import
// `DecisionLine` from this module. New code should use `DecisionPill`.
export const DecisionLine = DecisionPill;

function timeAgo(ms: number): string {
  const s = Math.round((Date.now() - ms) / 1000);
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return `${Math.round(s / 86400)}d ago`;
}

