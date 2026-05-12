import { useCallback, useEffect, useState } from "react";
import { RuleEditor } from "../components/RuleEditor";
import { ipc } from "../lib/ipc";
import type { ConfigDocument, InstalledBrowser, Rule } from "../lib/types";

interface Props {
  configEpoch: number;
}

type EditorState =
  | { kind: "closed" }
  | { kind: "new" }
  | { kind: "edit"; rule: Rule };

export function RulesPage({ configEpoch }: Props) {
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [editor, setEditor] = useState<EditorState>({ kind: "closed" });
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [nextDoc, nextBrowsers] = await Promise.all([
      ipc.configGet(),
      ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
    ]);
    setDoc(nextDoc);
    setBrowsers(nextBrowsers);
    setError(null);
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh, configEpoch]);

  const saveRule = async (rule: Rule) => {
    await ipc.ruleUpsert(rule);
    setEditor({ kind: "closed" });
    await refresh();
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
        Rules evaluated highest-priority first. Click <em>Edit</em> or{" "}
        <em>Add rule</em> to use the structured editor; advanced users can fall
        back to JSON below.
      </p>

      <div className="card">
        <div className="row">
          <h3 className="grow" style={{ margin: 0 }}>
            Rules ({doc?.rules.length ?? 0})
          </h3>
          <button
            className="primary"
            onClick={() => setEditor({ kind: "new" })}
            disabled={editor.kind !== "closed"}
          >
            + Add rule
          </button>
        </div>
        {doc && doc.rules.length === 0 && (
          <div className="empty">No rules yet — click “Add rule”.</div>
        )}
        {doc &&
          [...doc.rules]
            .sort((a, b) => b.priority - a.priority)
            .map((r) => (
              <RuleRow
                key={r.id}
                rule={r}
                onEdit={() => setEditor({ kind: "edit", rule: r })}
                onDelete={removeRule}
              />
            ))}
      </div>

      {editor.kind !== "closed" && (
        <RuleEditor
          initial={editor.kind === "edit" ? editor.rule : null}
          browsers={browsers}
          onSave={saveRule}
          onCancel={() => setEditor({ kind: "closed" })}
        />
      )}

      <div className="card">
        <div className="row">
          <h3 className="grow" style={{ margin: 0 }}>
            Default target
          </h3>
          <span className="mono muted">
            {doc?.default_target.browser}
            {doc?.default_target.profile ? ` / ${doc.default_target.profile}` : ""}
          </span>
        </div>
        <div className="muted">
          Fires when no rule matches. Change it in the <em>Settings</em> tab.
        </div>
      </div>

      <div className="card">
        <div className="row">
          <h3 className="grow" style={{ margin: 0 }}>
            Advanced: raw JSON
          </h3>
          <button onClick={() => setShowAdvanced((v) => !v)}>
            {showAdvanced ? "Hide" : "Show"}
          </button>
        </div>
        {showAdvanced && doc && <AdvancedJsonEditor doc={doc} onSaved={refresh} />}
      </div>

      {error && (
        <div className="card">
          <span className="tag danger">error</span>
          <span className="muted"> {error}</span>
        </div>
      )}
    </>
  );
}

function RuleRow({
  rule,
  onEdit,
  onDelete,
}: {
  rule: Rule;
  onEdit: () => void;
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
      <button onClick={onEdit}>Edit</button>
      <button className="danger" onClick={() => onDelete(rule)}>
        Delete
      </button>
    </div>
  );
}

function AdvancedJsonEditor({
  doc,
  onSaved,
}: {
  doc: ConfigDocument;
  onSaved: () => Promise<void>;
}) {
  const [draft, setDraft] = useState(() => JSON.stringify(doc, null, 2));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(JSON.stringify(doc, null, 2));
  }, [doc]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const parsed = JSON.parse(draft) as ConfigDocument;
      await ipc.configReplace(parsed);
      await onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
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
        <span className="grow" />
        <button
          onClick={() => setDraft(JSON.stringify(doc, null, 2))}
          disabled={busy}
        >
          Revert
        </button>
        <button className="primary" onClick={save} disabled={busy}>
          {busy ? "Saving…" : "Save"}
        </button>
      </div>
    </>
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
