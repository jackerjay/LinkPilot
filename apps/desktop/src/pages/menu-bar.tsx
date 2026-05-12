import { useCallback, useEffect, useState } from "react";
import { ipc, onRouteLogged } from "../lib/ipc";
import type { DoctorReport, RouteRecord, RoutingDecision } from "../lib/types";

interface Props {
  configEpoch: number;
}

export function MenuBarPage({ configEpoch }: Props) {
  const [doctor, setDoctor] = useState<DoctorReport | null>(null);
  const [recent, setRecent] = useState<RouteRecord[]>([]);
  const [testUrl, setTestUrl] = useState("https://github.com/anthropics/anthropic-cookbook");
  const [testDecision, setTestDecision] = useState<RoutingDecision | null>(null);

  const refresh = useCallback(async () => {
    setDoctor(await ipc.doctor());
    setRecent(await ipc.routeHistory(5));
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

  const runTest = async () => {
    try {
      setTestDecision(await ipc.routeEvaluate({ url: testUrl }));
    } catch (err) {
      setTestDecision({
        action: "block",
        reason: String(err),
      });
    }
  };

  return (
    <>
      <h2>Overview</h2>
      <p className="subtitle">Daemon status and the most recent routing decisions.</p>

      <div className="card">
        <h3>Status</h3>
        <div className="row">
          <span className="grow">Daemon version</span>
          <span className="mono muted">{doctor?.daemon_version ?? "…"}</span>
        </div>
        <div className="row">
          <span className="grow">LinkPilot is default browser</span>
          <span className={`tag ${doctor?.is_default_browser ? "ok" : "danger"}`}>
            {doctor?.is_default_browser ? "yes" : "no"}
          </span>
        </div>
        <div className="row">
          <span className="grow">Installed browsers detected</span>
          <span>{doctor?.installed_browser_count ?? 0}</span>
        </div>
        <div className="row">
          <span className="grow">Config file</span>
          <span className="mono muted">{doctor?.config_path ?? "…"}</span>
        </div>
      </div>

      <div className="card">
        <h3>Test a URL</h3>
        <div className="row">
          <input
            value={testUrl}
            onChange={(e) => setTestUrl(e.target.value)}
            placeholder="https://example.com"
          />
          <button className="primary" onClick={runTest}>
            Evaluate
          </button>
        </div>
        {testDecision && <DecisionLine decision={testDecision} />}
      </div>

      <div className="card">
        <h3>Recent routes</h3>
        {recent.length === 0 ? (
          <div className="empty">No routes yet. Try `lp open …` or click a link.</div>
        ) : (
          recent.map((r, i) => <RouteRow key={i} record={r} />)
        )}
      </div>
    </>
  );
}

export function DecisionLine({ decision }: { decision: RoutingDecision }) {
  if (decision.action === "open") {
    return (
      <div className="row">
        <span className="tag ok">open</span>
        <span className="grow mono">
          {decision.target.browser}
          {decision.target.profile ? ` / ${decision.target.profile}` : ""}
        </span>
        <span className="muted">{decision.reason}</span>
      </div>
    );
  }
  if (decision.action === "allow") {
    return (
      <div className="row">
        <span className="tag">allow</span>
        <span className="grow muted">{decision.reason}</span>
      </div>
    );
  }
  if (decision.action === "block") {
    return (
      <div className="row">
        <span className="tag danger">block</span>
        <span className="grow muted">{decision.reason}</span>
      </div>
    );
  }
  return (
    <div className="row">
      <span className="tag">ask</span>
      <span className="grow muted">{decision.reason}</span>
    </div>
  );
}

function RouteRow({ record }: { record: RouteRecord }) {
  const when = new Date(record.timestamp_ms).toLocaleTimeString();
  return (
    <div className="row">
      <span className="muted" style={{ width: 70 }}>
        {when}
      </span>
      <span className="grow mono" title={record.context.url}>
        {record.context.url}
      </span>
      <DecisionLine decision={record.decision} />
    </div>
  );
}
