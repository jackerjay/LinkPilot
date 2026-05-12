import { useCallback, useEffect, useState } from "react";
import { ipc } from "../lib/ipc";
import type { ConfigDocument, Rule } from "../lib/types";

interface Props {
  configEpoch: number;
}

export function RulesPage({ configEpoch }: Props) {
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [draft, setDraft] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    const next = await ipc.configGet();
    setDoc(next);
    setDraft(JSON.stringify(next, null, 2));
    setError(null);
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh, configEpoch]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const parsed = JSON.parse(draft) as ConfigDocument;
      await ipc.configReplace(parsed);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const removeRule = async (rule: Rule) => {
    try {
      await ipc.ruleDelete(rule.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <>
      <h2>Rules</h2>
      <p className="subtitle">
        Rules evaluated highest-priority first. Edit the JSON below to add /
        modify rules; saving rewrites the config file and broadcasts to other
        clients.
      </p>

      <div className="card">
        <h3>Quick list ({doc?.rules.length ?? 0})</h3>
        {doc && doc.rules.length === 0 && (
          <div className="empty">No rules yet — add some in the JSON editor.</div>
        )}
        {doc &&
          [...doc.rules]
            .sort((a, b) => b.priority - a.priority)
            .map((r) => <RuleRow key={r.id} rule={r} onDelete={removeRule} />)}
      </div>

      <div className="card">
        <h3>JSON editor</h3>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          spellCheck={false}
        />
        {error && (
          <div className="row">
            <span className="tag danger">error</span>
            <span className="muted grow">{error}</span>
          </div>
        )}
        <div className="row">
          <span className="grow muted">
            Default target:{" "}
            <span className="mono">
              {doc?.default_target.browser}
              {doc?.default_target.profile ? ` / ${doc.default_target.profile}` : ""}
            </span>
          </span>
          <button onClick={refresh} disabled={busy}>
            Revert
          </button>
          <button className="primary" onClick={save} disabled={busy}>
            {busy ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </>
  );
}

function RuleRow({
  rule,
  onDelete,
}: {
  rule: Rule;
  onDelete: (rule: Rule) => void;
}) {
  return (
    <div className="row">
      <span className="muted" style={{ width: 50 }}>
        #{rule.priority}
      </span>
      <span className="grow mono" title={JSON.stringify(rule.when)}>
        {describeWhen(rule.when)}
      </span>
      <span className="mono muted">{describeAction(rule.then)}</span>
      {!rule.enabled && <span className="tag danger">disabled</span>}
      {rule.source === "ts-compiled" && <span className="tag">ts</span>}
      <button className="danger" onClick={() => onDelete(rule)}>
        Delete
      </button>
    </div>
  );
}

function describeWhen(t: Rule["when"]): string {
  switch (t.op) {
    case "always":
      return "always";
    case "url-host":
      return `host ${t.pattern}`;
    case "url-path":
      return `path ${t.pattern}`;
    case "source-app":
      return `from app ${t.name}`;
    case "source-browser":
      return `from browser ${t.browser}`;
    case "source-profile":
      return `from profile ${t.profile}`;
    case "all":
      return t.of.map(describeWhen).join(" AND ");
    case "any":
      return t.of.map(describeWhen).join(" OR ");
    case "not":
      return `NOT (${describeWhen(t.of)})`;
  }
}

function describeAction(a: Rule["then"]): string {
  switch (a.kind) {
    case "open":
      return `open → ${a.target.browser}${a.target.profile ? "/" + a.target.profile : ""}`;
    case "keep-source":
      return "keep source";
    case "ask":
      return "ask";
    case "block":
      return "block";
  }
}
