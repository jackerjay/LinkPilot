import { useCallback, useEffect, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import type { DragEvent, MouseEvent as ReactMouseEvent } from "react";
import {
  Check,
  CopyPlus,
  FolderInput,
  GripVertical,
  Pencil,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import { BrowserBadge } from "@/components/BrowserBadge";
import { WhenDisplay } from "@/components/WhenDisplay";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { RuleEditor } from "@/components/RuleEditor";
import { ipc, onRouteLogged } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import type {
  ConfigDocument,
  InstalledBrowser,
  RouteRecord,
  Rule,
  Workspace,
} from "@/lib/types";

interface Props {
  configEpoch: number;
  /**
   * Workspace filter pushed in from the sidebar — when the user clicks
   * a workspace in App.tsx we tab-switch here AND want the rule list
   * pre-filtered. `token` bumps on every click so re-clicking the same
   * workspace re-applies the filter (resetting any local "all" override
   * the user toggled in the meantime).
   */
  pendingFilter?: { filter: string; token: number } | null;
}

type EditorState =
  | { kind: "closed" }
  | { kind: "new" }
  | { kind: "edit"; rule: Rule };

type DropPos = "before" | "after";

/// Filter applied to the rules list.
///   "all"        — show every rule (default; the only mode that allows drag-reorder)
///   "ungrouped"  — only rules with no workspace_id
///   <ws-id>      — only rules belonging to that workspace
type RuleFilter = "all" | "ungrouped" | string;

export function RulesPage({ configEpoch, pendingFilter }: Props) {
  const { t } = useTranslation("rules");
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [editor, setEditor] = useState<EditorState>({ kind: "closed" });
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<RuleFilter>("all");
  const [routeHistory, setRouteHistory] = useState<RouteRecord[]>([]);

  // Adopt the sidebar's filter request whenever the token bumps. Driven
  // by `token` rather than `filter` so re-clicking the same workspace
  // (after the user manually switched back to "all") re-applies the
  // filter — a value-only dep would no-op.
  useEffect(() => {
    if (pendingFilter) setFilter(pendingFilter.filter);
  }, [pendingFilter?.token]); // eslint-disable-line react-hooks/exhaustive-deps

  // Drag-to-reorder: ref is source of truth (no React stale-closure, no
  // WKWebView dataTransfer.types issues during dragover); state drives
  // the visual indicator only.
  const draggedIdRef = useRef<string | null>(null);
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const [dropPos, setDropPos] = useState<DropPos | null>(null);

  const refresh = useCallback(async () => {
    const [nextDoc, nextBrowsers, nextHistory] = await Promise.all([
      ipc.configGet(),
      ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
      // Route history powers the per-workspace hit count shown next to
      // each row in WorkspacesCard. 200 records mirrors what App.tsx
      // fetches for the sidebar so the two views stay numerically
      // consistent.
      ipc.routeHistory(200).catch(() => [] as RouteRecord[]),
    ]);
    setDoc(nextDoc);
    setBrowsers(nextBrowsers);
    setRouteHistory(nextHistory);
    setError(null);
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh, configEpoch]);

  // Stream live route events so the hit counters tick up in real time
  // while the user is on the Rules tab.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((rec) => {
      setRouteHistory((prev) => [rec, ...prev].slice(0, 200));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  // Build rule → workspace, then count matches per workspace. O(rules +
  // routes) per render; tiny and avoids stale memoization.
  const workspaceHits = new Map<string, number>();
  if (doc) {
    const ruleToWorkspace = new Map<string, string | null>();
    for (const r of doc.rules) {
      ruleToWorkspace.set(r.id, r.workspace_id ?? null);
    }
    for (const rec of routeHistory) {
      if (!rec.matched_rule) continue;
      const wsId = ruleToWorkspace.get(rec.matched_rule);
      if (!wsId) continue;
      workspaceHits.set(wsId, (workspaceHits.get(wsId) ?? 0) + 1);
    }
  }

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

  // Escape hatch for `source: "ts-compiled"` rules. The next `lp config
  // compile` run will rewrite every TsCompiled rule from .ts, but a
  // GUI-source copy survives untouched — so a user who needs to tweak
  // one rule without re-editing their .ts file can `Copy` it here and
  // then `Edit` the copy normally.
  //
  // The copy keeps the original rule intact (next compile still rewrites
  // it). Both rules then live side-by-side until the user either deletes
  // the original from .ts or deletes the GUI copy from here. Priorities
  // are preserved to keep router behaviour unchanged after copy.
  const copyRuleToGui = async (rule: Rule) => {
    if (rule.source !== "ts-compiled") return;
    const copy: Rule = {
      ...rule,
      id: crypto.randomUUID(),
      source: "gui",
      note: rule.note
        ? t("copyToGui.noteWithExisting", { note: rule.note })
        : t("copyToGui.note"),
    };
    try {
      await ipc.ruleUpsert(copy);
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

  // List order IS priority. Drag-reorder just rewrites the rules
  // array in the new order — no numeric priority field to maintain.
  const commitReorder = async (orderedIds: string[]) => {
    if (!doc) return;
    const byId = new Map(doc.rules.map((r) => [r.id, r] as const));
    const next: Rule[] = orderedIds.map((id) => byId.get(id)!);
    try {
      await ipc.configReplace({ ...doc, rules: next });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const sorted = doc ? doc.rules : [];

  const visibleRules = sorted.filter((r) => {
    if (filter === "all") return true;
    if (filter === "ungrouped") return !r.workspace_id;
    return r.workspace_id === filter;
  });

  // Drag-to-reorder is global (priority is global). Allowing it while a
  // filter is active would silently re-order rules the user can't see —
  // disable it in any filtered view.
  const reorderEnabled = filter === "all";

  // Quick-move: from a row's "move to workspace" popover, flip
  // `workspace_id` and persist via ruleUpsert (cheaper than rewriting
  // the whole document like commitReorder does).
  const moveRuleToWorkspace = async (
    rule: Rule,
    workspaceId: string | null,
  ) => {
    if (rule.workspace_id === workspaceId) return;
    try {
      await ipc.ruleUpsert({ ...rule, workspace_id: workspaceId });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="space-y-4">
      <header>
        <h2 className="mac-h2">{t("title")}</h2>
        <p className="mac-subtitle">
          <Trans
            i18nKey="subtitle"
            ns="rules"
            components={{ edit: <em />, add: <em /> }}
          />
        </p>
      </header>

      {doc && (
        <WorkspacesCard
          doc={doc}
          activeFilter={filter}
          hits={workspaceHits}
          onFilterChange={setFilter}
          onChanged={refresh}
          onError={(e) => setError(e)}
        />
      )}

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>
            {filter === "all"
              ? t("list.card", { count: visibleRules.length })
              : t("list.cardFiltered", {
                  visible: visibleRules.length,
                  total: doc?.rules.length ?? 0,
                })}
          </CardTitle>
          <Button
            onClick={() => setEditor({ kind: "new" })}
            disabled={editor.kind !== "closed"}
          >
            <Plus />
            {t("list.addRule")}
          </Button>
        </CardHeader>
        <CardContent className="space-y-3">
          {doc && (
            <FilterChips
              doc={doc}
              active={filter}
              onChange={setFilter}
            />
          )}
          {doc && doc.rules.length === 0 && (
            <div className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
              <Trans
                i18nKey="list.empty"
                ns="rules"
                components={{ add: <span /> }}
              />
            </div>
          )}
          {doc && doc.rules.length > 0 && visibleRules.length === 0 && (
            <div className="rounded-md border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
              {t("list.emptyFilterPrefix")}{" "}
              <button
                type="button"
                className="underline underline-offset-2 hover:text-foreground"
                onClick={() => setFilter("all")}
              >
                {t("list.showAll")}
              </button>
              .
            </div>
          )}
          {doc && visibleRules.length > 0 && (
            <div className="divide-y divide-border">
              {visibleRules.map((r) => {
                const isDragged = draggedId === r.id;
                const isDropBefore =
                  dropTargetId === r.id && dropPos === "before" && !isDragged;
                const isDropAfter =
                  dropTargetId === r.id && dropPos === "after" && !isDragged;
                const ws = r.workspace_id
                  ? doc.workspaces.find((w) => w.id === r.workspace_id) ?? null
                  : null;
                const wsDisabled = ws ? !ws.enabled : false;
                // Position in the global priority list (1 = top). Even
                // when a filter narrows what's shown, the number we
                // render is the rule's slot in `doc.rules`.
                const position = doc.rules.findIndex((x) => x.id === r.id) + 1;
                return (
                  <RuleRow
                    key={r.id}
                    rule={r}
                    position={position}
                    workspace={ws}
                    workspaceDisabled={wsDisabled}
                    workspaces={doc.workspaces}
                    reorderEnabled={reorderEnabled}
                    isDragged={isDragged}
                    isDropBefore={isDropBefore}
                    isDropAfter={isDropAfter}
                    onEdit={() => setEditor({ kind: "edit", rule: r })}
                    onDelete={() => removeRule(r)}
                    onCopyToGui={() => copyRuleToGui(r)}
                    onBadgeClick={() =>
                      setFilter(r.workspace_id ?? "ungrouped")
                    }
                    onMoveToWorkspace={(wsId) => moveRuleToWorkspace(r, wsId)}
                    onDragStart={(e) => {
                      if (!reorderEnabled) {
                        e.preventDefault();
                        return;
                      }
                      draggedIdRef.current = r.id;
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", r.id);
                      setDraggedId(r.id);
                    }}
                    onDragEnter={(e) => {
                      if (!reorderEnabled) return;
                      const src = draggedIdRef.current;
                      if (!src || src === r.id) return;
                      e.preventDefault();
                    }}
                    onDragOver={(e) => {
                      if (!reorderEnabled) return;
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
          workspaces={doc?.workspaces ?? []}
          onSave={saveRule}
          onCancel={() => setEditor({ kind: "closed" })}
        />
      )}

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>{t("defaultTarget.card")}</CardTitle>
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
            <Trans
              i18nKey="defaultTarget.description"
              ns="rules"
              components={{ settings: <em /> }}
            />
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>{t("advanced.card")}</CardTitle>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowAdvanced((v) => !v)}
          >
            {showAdvanced ? t("advanced.hide") : t("advanced.show")}
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
            <Badge variant="destructive">{t("advanced.errorTag")}</Badge>
            <span className="text-sm text-muted-foreground">{error}</span>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

interface RuleRowProps {
  rule: Rule;
  /** 1-based position in the global priority list (1 = top wins). */
  position: number;
  workspace: Workspace | null;
  workspaceDisabled: boolean;
  workspaces: Workspace[];
  reorderEnabled: boolean;
  isDragged: boolean;
  isDropBefore: boolean;
  isDropAfter: boolean;
  onEdit: () => void;
  onDelete: () => void;
  /** Clone a ts-compiled rule as a new GUI-editable rule. The original
   *  stays put (next `lp config compile` keeps overwriting it); the copy
   *  is the user's editable handle. Only invoked from rows whose
   *  `rule.source === "ts-compiled"`. */
  onCopyToGui: () => void;
  onBadgeClick: () => void;
  onMoveToWorkspace: (workspaceId: string | null) => void;
  onDragStart: (e: DragEvent<HTMLDivElement>) => void;
  onDragEnter: (e: DragEvent<HTMLDivElement>) => void;
  onDragOver: (e: DragEvent<HTMLDivElement>) => void;
  onDrop: (e: DragEvent<HTMLDivElement>) => void;
  onDragEnd: (e: DragEvent<HTMLDivElement>) => void;
}

function RuleRow({
  rule,
  position,
  workspace,
  workspaceDisabled,
  workspaces,
  reorderEnabled,
  isDragged,
  isDropBefore,
  isDropAfter,
  onEdit,
  onDelete,
  onCopyToGui,
  onBadgeClick,
  onMoveToWorkspace,
  onDragStart,
  onDragEnter,
  onDragOver,
  onDrop,
  onDragEnd,
}: RuleRowProps) {
  const { t } = useTranslation("rules");
  // Visually mute rules that won't fire — either explicitly disabled or
  // sitting in a turned-off workspace. The router treats both the same.
  const muted = !rule.enabled || workspaceDisabled;
  // ts-compiled rules are authored in `linkpilot.config.ts` and rewritten
  // every `lp config compile`. Direct edits / deletes from the GUI would
  // be silently undone on the next compile — make that obvious by
  // disabling the controls and offering the Copy-to-GUI escape hatch.
  const isCompiled = rule.source === "ts-compiled";
  return (
    <div
      draggable={reorderEnabled}
      onDragStart={onDragStart}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
      className={cn(
        "relative flex select-none items-center gap-2 py-2 transition-opacity",
        isDragged && "opacity-30",
        muted && !isDragged && "opacity-50",
        // Drop indicators: 2px primary-colored bar above/below the row.
        isDropBefore &&
          "before:pointer-events-none before:absolute before:inset-x-0 before:-top-px before:h-0.5 before:bg-primary",
        isDropAfter &&
          "after:pointer-events-none after:absolute after:inset-x-0 after:-bottom-px after:h-0.5 after:bg-primary",
      )}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <GripVertical
            className={cn(
              "h-4 w-4 shrink-0 text-muted-foreground",
              reorderEnabled ? "cursor-grab" : "cursor-not-allowed opacity-40",
            )}
            aria-hidden
          />
        </TooltipTrigger>
        {!reorderEnabled && (
          <TooltipContent>
            {t("row.dragDisabled")}
          </TooltipContent>
        )}
      </Tooltip>
      <span
        className="w-10 shrink-0 font-mono text-xs text-muted-foreground"
        title={t("row.priorityTitle")}
      >
        #{position}
      </span>
      <span
        className="flex-1 min-w-0 truncate text-xs"
        title={JSON.stringify(rule.when)}
      >
        <WhenDisplay matcher={rule.when} />
      </span>
      <span className="text-xs text-muted-foreground">
        <ActionDisplay action={rule.then} />
      </span>
      {workspace && (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={onBadgeClick}
              className="cursor-pointer"
              title={t("row.filterByWorkspaceTitle")}
            >
              <Badge
                variant={workspaceDisabled ? "outline" : "secondary"}
                className="font-normal hover:bg-accent"
              >
                {workspace.display_name}
              </Badge>
            </button>
          </TooltipTrigger>
          <TooltipContent>
            {t("row.filterByWorkspace", { name: workspace.display_name })}
            {workspaceDisabled && ` (${t("row.workspaceOff")})`}
          </TooltipContent>
        </Tooltip>
      )}
      {!rule.enabled && (
        <Badge variant="destructive">{t("row.disabled")}</Badge>
      )}
      {isCompiled && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Badge variant="secondary" className="cursor-help">
              {t("row.compiled")}
            </Badge>
          </TooltipTrigger>
          <TooltipContent>
            <Trans
              i18nKey="row.compiledHelp"
              ns="rules"
              components={{ code: <code />, compile: <code /> }}
            />
          </TooltipContent>
        </Tooltip>
      )}
      <QuickMoveButton
        currentWorkspaceId={rule.workspace_id ?? null}
        workspaces={workspaces}
        onSelect={onMoveToWorkspace}
      />
      {isCompiled && (
        <Tooltip>
          <TooltipTrigger asChild>
            {/* Wrap the disabled button in a span so the tooltip still
                fires — disabled buttons don't emit pointer events. */}
            <span tabIndex={0}>
              <Button variant="ghost" size="icon" disabled>
                <Pencil />
              </Button>
            </span>
          </TooltipTrigger>
          <TooltipContent>
            <Trans
              i18nKey="row.compiledReadonly"
              ns="rules"
              components={{ code: <code />, compile: <code /> }}
            />
          </TooltipContent>
        </Tooltip>
      )}
      {!isCompiled && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onEdit}>
              <Pencil />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("row.editRule")}</TooltipContent>
        </Tooltip>
      )}
      {isCompiled && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onCopyToGui}>
              <CopyPlus />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <Trans
              i18nKey="row.copyHelp"
              ns="rules"
              components={{ compile: <code /> }}
            />
          </TooltipContent>
        </Tooltip>
      )}
      {isCompiled && (
        <Tooltip>
          <TooltipTrigger asChild>
            <span tabIndex={0}>
              <Button
                variant="ghost"
                size="icon"
                disabled
                className="text-muted-foreground"
              >
                <Trash2 />
              </Button>
            </span>
          </TooltipTrigger>
          <TooltipContent>
            <Trans
              i18nKey="row.compiledDeleteHelp"
              ns="rules"
              components={{ code: <code />, compile: <code /> }}
            />
          </TooltipContent>
        </Tooltip>
      )}
      {!isCompiled && (
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
          <TooltipContent>{t("row.deleteRule")}</TooltipContent>
        </Tooltip>
      )}
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
  const { t } = useTranslation("rules");
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
          <Badge variant="destructive">{t("advanced.errorTag")}</Badge>
          <span className="text-xs text-muted-foreground">{error}</span>
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button
          variant="outline"
          onClick={() => setDraft(JSON.stringify(doc, null, 2))}
          disabled={busy}
        >
          {t("advanced.revert")}
        </Button>
        <Button onClick={save} disabled={busy}>
          {busy ? t("advanced.saving") : t("advanced.save")}
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Workspaces — named groups of rules with a batch on/off switch. The
// Rust router treats `workspace.enabled = false` exactly like
// `rule.enabled = false` for every rule whose `workspace_id` points at
// this workspace.

interface WorkspacesCardProps {
  doc: ConfigDocument;
  activeFilter: RuleFilter;
  /** workspace.id → hit count from recent route history. */
  hits: Map<string, number>;
  onFilterChange: (next: RuleFilter) => void;
  onChanged: () => Promise<void>;
  onError: (msg: string) => void;
}

function WorkspacesCard({
  doc,
  activeFilter,
  hits,
  onFilterChange,
  onChanged,
  onError,
}: WorkspacesCardProps) {
  const { t } = useTranslation("rules");
  const [creating, setCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  const counts = new Map<string, number>();
  for (const r of doc.rules) {
    if (r.workspace_id) {
      counts.set(r.workspace_id, (counts.get(r.workspace_id) ?? 0) + 1);
    }
  }

  const toggle = async (ws: Workspace) => {
    try {
      await ipc.workspaceSetEnabled(ws.id, !ws.enabled);
      await onChanged();
    } catch (e) {
      onError(String(e));
    }
  };

  const remove = async (ws: Workspace) => {
    try {
      await ipc.workspaceDelete(ws.id);
      await onChanged();
    } catch (e) {
      onError(String(e));
    }
  };

  const commitCreate = async () => {
    const name = draftName.trim();
    if (!name) {
      setCreating(false);
      setDraftName("");
      return;
    }
    try {
      await ipc.workspaceUpsert({
        id: crypto.randomUUID(),
        display_name: name,
        description: null,
        enabled: true,
      });
      setCreating(false);
      setDraftName("");
      await onChanged();
    } catch (e) {
      onError(String(e));
    }
  };

  const commitRename = async (ws: Workspace) => {
    const name = renameValue.trim();
    setRenamingId(null);
    if (!name || name === ws.display_name) return;
    try {
      await ipc.workspaceUpsert({ ...ws, display_name: name });
      await onChanged();
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0">
        <CardTitle>
          {t("workspaces.card", { count: doc.workspaces.length })}
        </CardTitle>
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            setCreating(true);
            setDraftName("");
          }}
          disabled={creating}
        >
          <Plus />
          {t("workspaces.new")}
        </Button>
      </CardHeader>
      <CardContent>
        {doc.workspaces.length === 0 && !creating && (
          <p className="text-xs text-muted-foreground">
            <Trans
              i18nKey="workspaces.empty"
              ns="rules"
              components={{ work: <em />, side: <em /> }}
            />
          </p>
        )}
        {(doc.workspaces.length > 0 || creating) && (
          <div className="divide-y divide-border">
            {doc.workspaces.map((ws) => {
              const count = counts.get(ws.id) ?? 0;
              const isRenaming = renamingId === ws.id;
              const isActive = activeFilter === ws.id;
              // Click anywhere on the row that isn't a control to enter
              // filter mode for that workspace; click again to clear.
              const handleRowClick = (e: ReactMouseEvent<HTMLDivElement>) => {
                // Don't fight the inline rename / delete / toggle.
                const target = e.target as HTMLElement;
                if (target.closest("button,input")) return;
                onFilterChange(isActive ? "all" : ws.id);
              };
              return (
                <div
                  key={ws.id}
                  onClick={handleRowClick}
                  className={cn(
                    "flex cursor-pointer items-center gap-3 rounded-md px-2 py-2 transition-colors",
                    isActive
                      ? "bg-accent/60 ring-1 ring-primary/30"
                      : "hover:bg-accent/30",
                  )}
                >
                  <MiniSwitch
                    checked={ws.enabled}
                    onChange={() => toggle(ws)}
                    label={t("workspaces.toggle", { name: ws.display_name })}
                  />
                  {isRenaming ? (
                    <Input
                      autoFocus
                      value={renameValue}
                      onChange={(e) => setRenameValue(e.target.value)}
                      onBlur={() => commitRename(ws)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") commitRename(ws);
                        if (e.key === "Escape") setRenamingId(null);
                      }}
                      className="h-7 max-w-[220px] text-sm"
                    />
                  ) : (
                    <button
                      type="button"
                      onClick={() => {
                        setRenamingId(ws.id);
                        setRenameValue(ws.display_name);
                      }}
                      className={cn(
                        "text-sm hover:underline",
                        !ws.enabled && "text-muted-foreground",
                      )}
                      title={t("workspaces.renameTitle")}
                    >
                      {ws.display_name}
                    </button>
                  )}
                  <span className="text-xs text-muted-foreground">
                    {t("counts.rule", { count })}
                  </span>
                  {/* Hits chip — recent routes attributed to a rule that
                      belongs to this workspace. Hidden at 0 so a fresh
                      workspace doesn't broadcast "no traffic yet". */}
                  {(hits.get(ws.id) ?? 0) > 0 && (
                    <span
                      className="text-xs text-muted-foreground font-mono"
                      title={t("workspaces.hitsTitle")}
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      · {t("counts.hit", { count: hits.get(ws.id) ?? 0 })}
                    </span>
                  )}
                  <div className="flex-1" />
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => remove(ws)}
                        className="text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t("workspaces.delete")}
                    </TooltipContent>
                  </Tooltip>
                </div>
              );
            })}
            {creating && (
              <div className="flex items-center gap-3 py-2">
                <MiniSwitch checked disabled label="" onChange={() => {}} />
                <Input
                  autoFocus
                  placeholder={t("workspaces.placeholder")}
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitCreate();
                    if (e.key === "Escape") {
                      setCreating(false);
                      setDraftName("");
                    }
                  }}
                  className="h-7 max-w-[220px] text-sm"
                />
                <div className="flex-1" />
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={commitCreate}
                  className="text-muted-foreground hover:text-foreground"
                >
                  <Check />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={() => {
                    setCreating(false);
                    setDraftName("");
                  }}
                  className="text-muted-foreground hover:text-destructive"
                >
                  <X />
                </Button>
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Filter chips — narrows the rules list to All / Ungrouped / a specific
// workspace. Mirrors the workspace-row click filter in WorkspacesCard
// but keeps the active filter visible right above the rules table so
// the user always knows what they're looking at.

interface FilterChipsProps {
  doc: ConfigDocument;
  active: RuleFilter;
  onChange: (next: RuleFilter) => void;
}

function FilterChips({ doc, active, onChange }: FilterChipsProps) {
  const { t } = useTranslation("rules");
  if (doc.workspaces.length === 0) return null;
  const total = doc.rules.length;
  const ungroupedCount = doc.rules.filter((r) => !r.workspace_id).length;
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <FilterChip
        label={t("filter.all")}
        count={total}
        active={active === "all"}
        onClick={() => onChange("all")}
      />
      <FilterChip
        label={t("filter.ungrouped")}
        count={ungroupedCount}
        active={active === "ungrouped"}
        onClick={() => onChange("ungrouped")}
      />
      {doc.workspaces.map((ws) => {
        const count = doc.rules.filter((r) => r.workspace_id === ws.id).length;
        return (
          <FilterChip
            key={ws.id}
            label={ws.display_name}
            count={count}
            active={active === ws.id}
            dimmed={!ws.enabled}
            onClick={() => onChange(ws.id)}
          />
        );
      })}
    </div>
  );
}

function FilterChip({
  label,
  count,
  active,
  dimmed,
  onClick,
}: {
  label: string;
  count: number;
  active: boolean;
  dimmed?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs transition-colors",
        active
          ? "border-primary/50 bg-primary/10 text-foreground"
          : "border-border bg-transparent text-muted-foreground hover:bg-accent/40 hover:text-foreground",
        dimmed && !active && "opacity-60",
      )}
    >
      <span>{label}</span>
      <span
        className={cn(
          "rounded-full px-1.5 text-[10px] font-mono",
          active
            ? "bg-primary/20 text-foreground"
            : "bg-muted text-muted-foreground",
        )}
      >
        {count}
      </span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Quick-move — a tiny popover anchored to a rule row. Click the folder
// icon, pick a workspace (or "ungrouped"), and the rule's
// `workspace_id` flips immediately via `rule_upsert`. No full editor,
// no extra round-trip.
//
// Built inline instead of pulling Radix's Popover/DropdownMenu just for
// one widget. Closes on click-outside via a document listener and on
// Escape via keydown.

interface QuickMoveButtonProps {
  currentWorkspaceId: string | null;
  workspaces: Workspace[];
  onSelect: (workspaceId: string | null) => void;
}

function QuickMoveButton({
  currentWorkspaceId,
  workspaces,
  onSelect,
}: QuickMoveButtonProps) {
  const { t } = useTranslation("rules");
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: globalThis.MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const pick = (workspaceId: string | null) => {
    setOpen(false);
    onSelect(workspaceId);
  };

  return (
    <div ref={containerRef} className="relative">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setOpen((v) => !v)}
            className="text-muted-foreground hover:text-foreground"
          >
            <FolderInput />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("quickMove.button")}</TooltipContent>
      </Tooltip>
      {open && (
        <div
          className="absolute right-0 top-full z-30 mt-1 min-w-[180px] overflow-hidden rounded-md border border-border bg-popover py-1 text-popover-foreground shadow-md"
          role="menu"
        >
          <QuickMoveItem
            label={t("quickMove.ungrouped")}
            active={currentWorkspaceId === null}
            onClick={() => pick(null)}
            muted
          />
          {workspaces.length > 0 && (
            <div className="my-1 border-t border-border" />
          )}
          {workspaces.map((ws) => (
            <QuickMoveItem
              key={ws.id}
              label={ws.display_name}
              hint={!ws.enabled ? t("quickMove.off") : undefined}
              active={currentWorkspaceId === ws.id}
              onClick={() => pick(ws.id)}
            />
          ))}
          {workspaces.length === 0 && (
            <div className="px-3 py-1.5 text-xs text-muted-foreground">
              {t("quickMove.noWorkspaces")}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function QuickMoveItem({
  label,
  hint,
  active,
  onClick,
  muted,
}: {
  label: string;
  hint?: string;
  active: boolean;
  onClick: () => void;
  muted?: boolean;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className={cn(
        "flex w-full items-center justify-between gap-3 px-3 py-1.5 text-left text-xs transition-colors hover:bg-accent",
        active && "bg-accent/50",
        muted && "text-muted-foreground",
      )}
    >
      <span>{label}</span>
      <span className="flex items-center gap-2">
        {hint && <span className="text-muted-foreground">{hint}</span>}
        {active && <Check className="h-3.5 w-3.5" />}
      </span>
    </button>
  );
}

// macOS-flavoured pill switch. Inline because we don't yet need a
// generic ui/Switch primitive anywhere else — keeping the surface area
// of the design system small.
interface MiniSwitchProps {
  checked: boolean;
  onChange: () => void;
  label: string;
  disabled?: boolean;
}

function MiniSwitch({ checked, onChange, label, disabled }: MiniSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={onChange}
      className={cn(
        "relative inline-flex h-[18px] w-[30px] shrink-0 items-center rounded-full transition-colors",
        checked ? "bg-primary" : "bg-muted",
        disabled && "opacity-60 cursor-not-allowed",
      )}
    >
      <span
        className={cn(
          "inline-block h-[14px] w-[14px] rounded-full bg-white shadow transition-transform",
          checked ? "translate-x-[14px]" : "translate-x-[2px]",
        )}
      />
    </button>
  );
}

function ActionDisplay({ action }: { action: Rule["then"] }) {
  const { t } = useTranslation("rules");
  switch (action.kind) {
    case "open":
      return (
        <span className="inline-flex items-center gap-1.5">
          <span>{t("action.open")}</span>
          <BrowserBadge
            browserId={action.target.browser}
            profile={action.target.profile}
          />
        </span>
      );
    case "keep-source":
      return <span>{t("action.keepSource")}</span>;
    case "ask":
      return <span>{t("action.ask")}</span>;
    case "block":
      return <span>{t("action.block")}</span>;
  }
}
