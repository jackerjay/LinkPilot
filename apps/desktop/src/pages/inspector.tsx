import { useCallback, useEffect, useState } from "react";
import { ExplanationView } from "../components/Explanation";
import { ipc, onRouteLogged } from "../lib/ipc";
import { DecisionLine } from "./menu-bar";
import type { ConfigDocument, RouteRecord, Rule } from "../lib/types";

export function InspectorPage() {
  const [records, setRecords] = useState<RouteRecord[]>([]);
  const [selected, setSelected] = useState<RouteRecord | null>(null);
  const [config, setConfig] = useState<ConfigDocument | null>(null);
  const [showRaw, setShowRaw] = useState(false);

  const refresh = useCallback(async () => {
    const [recs, doc] = await Promise.all([
      ipc.routeHistory(200),
      ipc.configGet(),
    ]);
    setRecords(recs);
    setConfig(doc);
  }, []);

  useEffect(() => {
    refresh().catch(console.error);
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((record) => {
      setRecords((prev) => [record, ...prev].slice(0, 200));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const matchedRule =
    selected && selected.matched_rule
      ? config?.rules.find((r) => r.id === selected.matched_rule) ?? null
      : null;

  return (
    <>
      <h2>Route Inspector</h2>
      <p className="subtitle">
        Every decision LinkPilot makes, newest first. Click a row to see why
        the rule matched.
      </p>

      <div className="card scroll">
        {records.length === 0 ? (
          <div className="empty">
            No routes logged yet. Click some links or run{" "}
            <span className="mono">lp open …</span> while the daemon is up.
          </div>
        ) : (
          records.map((r, i) => (
            <div
              key={i}
              className="row"
              style={{
                cursor: "pointer",
                background: selected === r ? "var(--accent-soft)" : undefined,
              }}
              onClick={() => setSelected(r)}
            >
              <span className="muted" style={{ width: 80 }}>
                {new Date(r.timestamp_ms).toLocaleTimeString()}
              </span>
              <span className="grow mono">{r.context.url}</span>
              <span className="muted">
                {r.context.source.app_name ?? r.context.source.type}
              </span>
              <DecisionLine decision={r.decision} />
            </div>
          ))
        )}
      </div>

      {selected && (
        <div className="card">
          <h3>Selected route</h3>
          <RouteSummary record={selected} matchedRule={matchedRule} />

          <div className="rule-editor-section">
            <div className="muted">Why this decision</div>
            <ExplanationView
              explanation={selected.explanation}
              emptyMessage="No rule fired. The route fell back to the configured default_target."
            />
          </div>

          <div className="row">
            <span className="grow muted">
              Raw record (for debugging / bug reports)
            </span>
            <button onClick={() => setShowRaw((v) => !v)}>
              {showRaw ? "Hide" : "Show"}
            </button>
          </div>
          {showRaw && (
            <pre className="mono" style={{ whiteSpace: "pre-wrap", margin: 0 }}>
              {JSON.stringify(selected, null, 2)}
            </pre>
          )}
        </div>
      )}
    </>
  );
}

function RouteSummary({
  record,
  matchedRule,
}: {
  record: RouteRecord;
  matchedRule: Rule | null;
}) {
  return (
    <>
      <div className="row">
        <span className="muted" style={{ width: 80 }}>
          URL
        </span>
        <span className="grow mono">{record.context.url}</span>
      </div>
      <div className="row">
        <span className="muted" style={{ width: 80 }}>
          Source
        </span>
        <span className="grow">
          {record.context.source.app_name ? (
            <>
              <span className="mono">{record.context.source.app_name}</span>{" "}
              <span className="muted">({record.context.source.type})</span>
            </>
          ) : (
            <span className="muted">{record.context.source.type}</span>
          )}
        </span>
      </div>
      <div className="row">
        <span className="muted" style={{ width: 80 }}>
          Decision
        </span>
        <span className="grow">
          <DecisionLine decision={record.decision} />
        </span>
      </div>
      <div className="row">
        <span className="muted" style={{ width: 80 }}>
          Rule
        </span>
        <span className="grow">
          {matchedRule ? (
            <>
              <span className="mono">#{matchedRule.priority}</span>{" "}
              {matchedRule.note ? (
                <span>{matchedRule.note}</span>
              ) : (
                <span className="muted">(no note)</span>
              )}
            </>
          ) : (
            <span className="muted">— default target (no rule matched)</span>
          )}
        </span>
      </div>
    </>
  );
}

