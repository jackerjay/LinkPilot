import { useCallback, useEffect, useState } from "react";
import { ipc, onRouteLogged } from "../lib/ipc";
import { DecisionLine } from "./menu-bar";
import type { RouteRecord } from "../lib/types";

export function InspectorPage() {
  const [records, setRecords] = useState<RouteRecord[]>([]);
  const [selected, setSelected] = useState<RouteRecord | null>(null);

  const refresh = useCallback(async () => {
    setRecords(await ipc.routeHistory(200));
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

  return (
    <>
      <h2>Route Inspector</h2>
      <p className="subtitle">
        Every decision LinkPilot makes, newest first. Click a row to see the
        full routing context.
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
              style={{ cursor: "pointer" }}
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
          <pre className="mono" style={{ whiteSpace: "pre-wrap", margin: 0 }}>
            {JSON.stringify(selected, null, 2)}
          </pre>
        </div>
      )}
    </>
  );
}
