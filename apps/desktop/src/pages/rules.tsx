import { useCallback, useEffect, useRef, useState } from "react";
import type { DragEvent } from "react";
import { GripVertical, Pencil, Plus, Trash2 } from "lucide-react";
import { BrowserBadge } from "@/components/BrowserBadge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { RuleEditor } from "@/components/RuleEditor";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import type { ConfigDocument, InstalledBrowser, Rule } from "@/lib/types";

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

  // Drag-to-reorder: ref is source of truth (no React stale-closure, no
  // WKWebView dataTransfer.types issues during dragover); state drives
  // the visual indicator only.
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

  const clearVisuals = () => {
    setDraggedId(null);
    setDropTargetId(null);
    setDropPos(null);
  };
  const clearDrag = () => {
    draggedIdRef.current = null;
    clearVisuals();
  };

  // Restamp priorities N*10, (N-1)*10, … so the new top wins.
  const commitReorder = async (orderedIds: string[]) => {
    if (!doc) return;
    const byId = new Map(doc.rules.map((r) => [r.id, r] as const));
    const total = orderedIds.length;
    const next: Rule[] = orderedIds.map((id, idx) => ({
      ...byId.get(id)!,
      priority: (total - idx) * 10,
    }));
    try {
      await ipc.configReplace({ ...doc, rules: next });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const sorted = doc
    ? [...doc.rules].sort((a, b) => b.priority - a.priority)
    : [];

  return (
    <div className="space-y-4">
      <header>
        <h2 className="text-xl font-semibold tracking-tight">Rules</h2>
        <p className="text-sm text-muted-foreground">
          Evaluated highest-priority first. Drag a row to reorder; click{" "}
          <em>Edit</em> or <em>Add rule</em> for the structured editor.
        </p>
      </header>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>Rules ({doc?.rules.length ?? 0})</CardTitle>
          <Button
            onClick={() => setEditor({ kind: "new" })}
            disabled={editor.kind !== "closed"}
          >
            <Plus />
            Add rule
          </Button>
        </CardHeader>
        <CardContent>
          {doc && doc.rules.length === 0 && (
            <div className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
              No rules yet — click "Add rule".
            </div>
          )}
          {doc && doc.rules.length > 0 && (
            <div className="divide-y divide-border">
              {sorted.map((r) => {
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
                    onDelete={() => removeRule(r)}
                    onDragStart={(e) => {
                      draggedIdRef.current = r.id;
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", r.id);
                      setDraggedId(r.id);
                    }}
                    onDragEnter={(e) => {
                      const src = draggedIdRef.current;
                      if (!src || src === r.id) return;
                      e.preventDefault();
                    }}
                    onDragOver={(e) => {
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
                    onDragEnd={clearVisuals}
                  />
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      {editor.kind !== "closed" && (
        <RuleEditor
          initial={editor.kind === "edit" ? editor.rule : null}
          browsers={browsers}
          onSave={saveRule}
          onCancel={() => setEditor({ kind: "closed" })}
        />
      )}

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>Default target</CardTitle>
          {doc && (
            <span className="text-xs text-muted-foreground">
              <BrowserBadge
                browserId={doc.default_target.browser}
                profile={doc.default_target.profile}
              />
            </span>
          )}
        </CardHeader>
        <CardContent>
          <p className="text-xs text-muted-foreground">
            Fires when no rule matches. Change it in the <em>Settings</em> tab.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>Advanced: raw JSON</CardTitle>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowAdvanced((v) => !v)}
          >
            {showAdvanced ? "Hide" : "Show"}
          </Button>
        </CardHeader>
        {showAdvanced && doc && (
          <CardContent>
            <AdvancedJsonEditor doc={doc} onSaved={refresh} />
          </CardContent>
        )}
      </Card>

      {error && (
        <Card>
          <CardContent className="flex items-center gap-2 pt-4">
            <Badge variant="destructive">error</Badge>
            <span className="text-sm text-muted-foreground">{error}</span>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

interface RuleRowProps {
  rule: Rule;
  isDragged: boolean;
  isDropBefore: boolean;
  isDropAfter: boolean;
  onEdit: () => void;
  onDelete: () => void;
  onDragStart: (e: DragEvent<HTMLDivElement>) => void;
  onDragEnter: (e: DragEvent<HTMLDivElement>) => void;
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
  onDragEnter,
  onDragOver,
  onDrop,
  onDragEnd,
}: RuleRowProps) {
  return (
    <div
      draggable
      onDragStart={onDragStart}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
      className={cn(
        "relative flex select-none items-center gap-2 py-2 transition-opacity",
        isDragged && "opacity-30",
        // Drop indicators: 2px primary-colored bar above/below the row.
        isDropBefore &&
          "before:pointer-events-none before:absolute before:inset-x-0 before:-top-px before:h-0.5 before:bg-primary",
        isDropAfter &&
          "after:pointer-events-none after:absolute after:inset-x-0 after:-bottom-px after:h-0.5 after:bg-primary",
      )}
    >
      <GripVertical
        className="h-4 w-4 shrink-0 cursor-grab text-muted-foreground"
        aria-hidden
      />
      <span className="w-10 shrink-0 font-mono text-xs text-muted-foreground">
        #{rule.priority}
      </span>
      <span
        className="flex-1 truncate font-mono text-xs"
        title={JSON.stringify(rule.when)}
      >
        {describeWhen(rule.when)}
      </span>
      <span className="text-xs text-muted-foreground">
        <ActionDisplay action={rule.then} />
      </span>
      {!rule.enabled && <Badge variant="destructive">disabled</Badge>}
      {rule.source === "ts-compiled" && <Badge variant="secondary">ts</Badge>}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon" onClick={onEdit}>
            <Pencil />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Edit rule</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            onClick={onDelete}
            className="text-muted-foreground hover:text-destructive"
          >
            <Trash2 />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Delete rule</TooltipContent>
      </Tooltip>
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
    <div className="space-y-3">
      <Textarea
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        spellCheck={false}
        rows={16}
      />
      {error && (
        <div className="flex items-center gap-2">
          <Badge variant="destructive">error</Badge>
          <span className="text-xs text-muted-foreground">{error}</span>
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button
          variant="outline"
          onClick={() => setDraft(JSON.stringify(doc, null, 2))}
          disabled={busy}
        >
          Revert
        </Button>
        <Button onClick={save} disabled={busy}>
          {busy ? "Saving…" : "Save"}
        </Button>
      </div>
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

function ActionDisplay({ action }: { action: Rule["then"] }) {
  switch (action.kind) {
    case "open":
      return (
        <span className="inline-flex items-center gap-1.5">
          <span>open →</span>
          <BrowserBadge
            browserId={action.target.browser}
            profile={action.target.profile}
          />
        </span>
      );
    case "keep-source":
      return <span>keep source</span>;
    case "ask":
      return <span>ask</span>;
    case "block":
      return <span>block</span>;
  }
}
