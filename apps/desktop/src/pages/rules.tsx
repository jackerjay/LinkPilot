import { useCallback, useEffect, useRef, useState } from "react";
import type { DragEvent } from "react";
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

type DropPos = "before" | "after";

export function RulesPage({ configEpoch }: Props) {
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [editor, setEditor] = useState<EditorState>({ kind: "closed" });
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Drag-to-reorder. The ref is the source of truth across the entire
  // drag lifecycle (no React stale-closure, no WKWebView dataTransfer.types
  // limitations during dragover). State drives the visual indicator only —
  // a one-frame lag there is harmless.
  const draggedIdRef = useRef<string | null>(null);
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const [dropPos, setDropPos] = useState<DropPos | null>(null);

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

  // Visual indicator only — does NOT clear the ref. WKWebView occasionally
  // fires dragend before drop, and if dragend clears the ref the drop
  // handler bails out before commitReorder runs. The ref is cleared
  // explicitly inside onDrop after the commit (or its bailouts).
  const clearVisuals = () => {
    setDraggedId(null);
    setDropTargetId(null);
    setDropPos(null);
  };
  const clearDrag = () => {
    draggedIdRef.current = null;
    clearVisuals();
  };

  // Commit a reorder: assign priorities N*10 down to 10 in the new order
  // so the highest item wins. Step 10 leaves room to nudge a single rule
  // by editing its priority manually later.
  const commitReorder = async (orderedIds: string[]) => {
    if (!doc) {
      console.warn("[dnd] commit skipped: no doc");
      return;
    }
    const byId = new Map(doc.rules.map((r) => [r.id, r] as const));
    const total = orderedIds.length;
    const next: Rule[] = orderedIds.map((id, idx) => ({
      ...byId.get(id)!,
      priority: (total - idx) * 10,
    }));
    console.log(
      "[dnd] commit",
      next.map((r) => `${r.priority}=${r.id.slice(0, 8)}`),
    );
    try {
      await ipc.configReplace({ ...doc, rules: next });
      await refresh();
    } catch (e) {
      console.error("[dnd] commit failed", e);
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
        {doc && doc.rules.length > 1 && (
          <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>
            Drag a row to reorder. Highest priority wins; the priority
            number is restamped (steps of 10) after every drop.
          </div>
        )}
        {doc &&
          (() => {
            const sorted = [...doc.rules].sort(
              (a, b) => b.priority - a.priority,
            );
            return sorted.map((r) => {
              const isDragged = draggedId === r.id;
              const isDropBefore =
                dropTargetId === r.id && dropPos === "before" && !isDragged;
              const isDropAfter =
                dropTargetId === r.id && dropPos === "after" && !isDragged;
              return (
                <RuleRow
                  key={r.id}
                  rule={r}
                  isDragged={isDragged}
                  isDropBefore={isDropBefore}
                  isDropAfter={isDropAfter}
                  onEdit={() => setEditor({ kind: "edit", rule: r })}
                  onDelete={removeRule}
                  onDragStart={(e) => {
                    draggedIdRef.current = r.id;
                    e.dataTransfer.effectAllowed = "move";
                    // Firefox requires SOME payload to start the drag.
                    e.dataTransfer.setData("text/plain", r.id);
                    setDraggedId(r.id);
                    console.log("[dnd] start", r.id);
                  }}
                  onDragOver={(e) => {
                    // Use the ref (sync, no stale closure) and DON'T
                    // look at dataTransfer.types — WKWebView doesn't
                    // expose custom MIMEs during dragover, and standard
                    // ones would over-match (URL drags, text selection).
                    const src = draggedIdRef.current;
                    if (!src || src === r.id) return;
                    e.preventDefault();
                    e.dataTransfer.dropEffect = "move";
                    const rect = (
                      e.currentTarget as HTMLElement
                    ).getBoundingClientRect();
                    const mid = rect.top + rect.height / 2;
                    const pos: DropPos =
                      e.clientY < mid ? "before" : "after";
                    if (dropTargetId !== r.id) setDropTargetId(r.id);
                    if (dropPos !== pos) setDropPos(pos);
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    const sourceId = draggedIdRef.current;
                    console.log("[dnd] drop", { sourceId, target: r.id });
                    if (!sourceId || sourceId === r.id) {
                      clearDrag();
                      return;
                    }
                    const rect = (
                      e.currentTarget as HTMLElement
                    ).getBoundingClientRect();
                    const mid = rect.top + rect.height / 2;
                    const pos: DropPos =
                      e.clientY < mid ? "before" : "after";
                    const ids = sorted.map((s) => s.id);
                    const from = ids.indexOf(sourceId);
                    if (from < 0) {
                      clearDrag();
                      return;
                    }
                    const reordered = ids.filter((_, i) => i !== from);
                    let insertAt = reordered.indexOf(r.id);
                    if (pos === "after") insertAt += 1;
                    reordered.splice(insertAt, 0, sourceId);
                    clearDrag();
                    commitReorder(reordered).catch((err) =>
                      setError(String(err)),
                    );
                  }}
                  onDragEnd={() => {
                    console.log(
                      "[dnd] end (ref still:",
                      draggedIdRef.current,
                      ")",
                    );
                    // Clear visuals only; ref is cleared in onDrop. This
                    // avoids a WKWebView race where dragend fires before
                    // drop, nuking the ref the drop handler needs to read.
                    clearVisuals();
                  }}
                />
              );
            });
          })()}
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

interface RuleRowProps {
  rule: Rule;
  isDragged: boolean;
  isDropBefore: boolean;
  isDropAfter: boolean;
  onEdit: () => void;
  onDelete: (rule: Rule) => void;
  onDragStart: (e: DragEvent<HTMLDivElement>) => void;
  onDragOver: (e: DragEvent<HTMLDivElement>) => void;
  onDrop: (e: DragEvent<HTMLDivElement>) => void;
  onDragEnd: (e: DragEvent<HTMLDivElement>) => void;
}

function RuleRow({
  rule,
  isDragged,
  isDropBefore,
  isDropAfter,
  onEdit,
  onDelete,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
}: RuleRowProps) {
  const classes = ["row", "rule-row"];
  if (isDragged) classes.push("dragging");
  if (isDropBefore) classes.push("drop-before");
  if (isDropAfter) classes.push("drop-after");
  return (
    <div
      className={classes.join(" ")}
      draggable
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
    >
      <span
        className="muted drag-handle"
        title="Drag to reorder"
        style={{ cursor: "grab", userSelect: "none" }}
      >
        ⋮⋮
      </span>
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
