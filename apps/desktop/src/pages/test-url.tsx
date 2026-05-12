// Dry-run a URL through the live routing engine. No browser is launched;
// the daemon evaluates the rules and returns the decision + the full
// MatcherEval tree, which we render exactly like the Inspector does.
//
// Lets you test rules end-to-end without setting LinkPilot as default
// browser or hitting `lp open --dry-run` in a terminal.

import { useCallback, useEffect, useMemo, useState } from "react";
import { ExplanationView } from "../components/Explanation";
import { ipc } from "../lib/ipc";
import type {
  BrowserProfile,
  ConfigDocument,
  Explained,
  InstalledBrowser,
  Rule,
} from "../lib/types";
import { DecisionLine } from "./menu-bar";

interface Props {
  configEpoch: number;
}

export function TestUrlPage({ configEpoch }: Props) {
  const [url, setUrl] = useState("https://github.com/anthropics/anthropic-cookbook");
  const [fromApp, setFromApp] = useState("");
  const [fromBrowser, setFromBrowser] = useState("");
  const [fromProfile, setFromProfile] = useState("");

  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [config, setConfig] = useState<ConfigDocument | null>(null);

  const [result, setResult] = useState<Explained | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
      ipc.configGet(),
    ]).then(([b, c]) => {
      setBrowsers(b);
      setConfig(c);
    });
  }, [configEpoch]);

  useEffect(() => {
    if (!fromBrowser) {
      setProfiles([]);
      setFromProfile("");
      return;
    }
    let alive = true;
    ipc
      .listProfiles(fromBrowser)
      .then((p) => {
        if (alive) setProfiles(p);
      })
      .catch(() => {
        if (alive) setProfiles([]);
      });
    return () => {
      alive = false;
    };
  }, [fromBrowser]);

  // Debounced live evaluation: 250ms after the last change to any input,
  // ask the daemon. URL must be non-empty and parse-shaped (has a scheme).
  const evaluate = useCallback(async () => {
    if (!url.trim()) {
      setResult(null);
      setError(null);
      return;
    }
    try {
      const out = await ipc.routeEvaluate({
        url: url.trim(),
        from_app: fromApp.trim() || null,
        from_browser: fromBrowser || null,
        from_profile: fromProfile || null,
      });
      setResult(out);
      setError(null);
    } catch (e) {
      setResult(null);
      setError(String(e));
    }
  }, [url, fromApp, fromBrowser, fromProfile]);

  useEffect(() => {
    const t = setTimeout(() => {
      evaluate().catch(console.error);
    }, 250);
    return () => clearTimeout(t);
  }, [evaluate]);

  const matchedRule = useMemo<Rule | null>(() => {
    if (!result || !config) return null;
    if (result.decision.action !== "open") return null;
    const id = result.decision.matched_rule;
    if (!id) return null;
    return config.rules.find((r) => r.id === id) ?? null;
  }, [result, config]);

  return (
    <>
      <h2>Test URL</h2>
      <p className="subtitle">
        Run a URL through the live router without opening a browser. Edit
        a rule, switch back here, and the decision updates instantly.
      </p>

      <div className="card">
        <label>
          <div className="muted">URL</div>
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://example.com/some/path"
          />
        </label>

        <div className="row" style={{ gap: 12 }}>
          <label className="grow">
            <div className="muted">From app (optional)</div>
            <input
              type="text"
              value={fromApp}
              onChange={(e) => setFromApp(e.target.value)}
              placeholder="Slack, Terminal, VSCode…"
            />
          </label>
          <label className="grow">
            <div className="muted">From browser (optional)</div>
            <select
              value={fromBrowser}
              onChange={(e) => setFromBrowser(e.target.value)}
            >
              <option value="">— none —</option>
              {browsers.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.display_name}
                </option>
              ))}
            </select>
          </label>
          <label className="grow">
            <div className="muted">From profile (optional)</div>
            <select
              value={fromProfile}
              onChange={(e) => setFromProfile(e.target.value)}
              disabled={!fromBrowser}
            >
              <option value="">— any —</option>
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.display_name}
                </option>
              ))}
            </select>
          </label>
        </div>
      </div>

      {error && (
        <div className="card">
          <span className="tag danger">error</span>
          <span className="muted"> {error}</span>
        </div>
      )}

      {result && (
        <div className="card">
          <h3>Result</h3>
          <div className="row">
            <span className="muted" style={{ width: 80 }}>
              Decision
            </span>
            <span className="grow">
              <DecisionLine decision={result.decision} />
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

          <div className="rule-editor-section">
            <div className="muted">Why this decision</div>
            <ExplanationView
              explanation={result.explanation}
              emptyMessage="No rule fired. The route would fall back to the configured default_target."
            />
          </div>
        </div>
      )}
    </>
  );
}
