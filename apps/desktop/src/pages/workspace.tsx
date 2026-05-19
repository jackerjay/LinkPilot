// Workspace detail page — opened when the user clicks a workspace in
// the sidebar. Self-contained surface (not a Rules tab pre-filter)
// because the user needs a place where the workspace itself is the
// subject: its name, on/off state, the rules it owns, and the routing
// traffic it has attracted.

import { useCallback, useEffect, useState } from "react";
import { ArrowRight, Pencil, Trash2 } from "lucide-react";
import { BrowserBadge } from "@/components/BrowserBadge";
import { Input } from "@/components/ui/input";
import { ipc, onRouteLogged } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import type {
  ConfigDocument,
  RouteRecord,
  Rule,
  Workspace,
} from "@/lib/types";

interface Props {
  workspaceId: string;
  configEpoch: number;
  /** Called when the user wants to edit the rules in this workspace —
   *  switches to the Rules tab pre-filtered to this id. */
  onOpenRulesFiltered: (workspaceId: string) => void;
  /** Called after deletion so the host can pop us back to a sane view. */
  onDeleted: () => void;
}

export function WorkspacePage({
  workspaceId,
  configEpoch,
  onOpenRulesFiltered,
  onDeleted,
}: Props) {
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [history, setHistory] = useState<RouteRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");

  const refresh = useCallback(async () => {
    try {
      const [nextDoc, nextHistory] = await Promise.all([
        ipc.configGet(),
        ipc.routeHistory(200).catch(() => [] as RouteRecord[]),
      ]);
      setDoc(nextDoc);
      setHistory(nextHistory);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh, configEpoch]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((rec) => {
      setHistory((prev) => [rec, ...prev].slice(0, 200));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const workspace: Workspace | undefined = doc?.workspaces.find(
    (w) => w.id === workspaceId,
  );

  // Defensive: workspace was deleted (from Rules tab or via config
  // reload) while this page was open. Show a brief stub + back button.
  // The host (App.tsx) is responsible for actually navigating away.
  if (!workspace && doc) {
    return (
      <div>
        <h2 className="mac-h2">Workspace</h2>
        <p className="mac-subtitle">
          This workspace no longer exists. It may have been deleted from
          the Rules tab.
        </p>
      </div>
    );
  }

  if (!workspace || !doc) {
    return (
      <div>
        <h2 className="mac-h2">Workspace</h2>
        <p className="mac-subtitle">Loading…</p>
      </div>
    );
  }

  const rules = doc.rules.filter((r) => r.workspace_id === workspace.id);
  const enabledRules = rules.filter((r) => r.enabled).length;
  const ruleIds = new Set(rules.map((r) => r.id));
  const hitRecords = history.filter(
    (r) => r.matched_rule && ruleIds.has(r.matched_rule),
  );

  const toggle = async () => {
    try {
      await ipc.workspaceSetEnabled(workspace.id, !workspace.enabled);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const remove = async () => {
    try {
      await ipc.workspaceDelete(workspace.id);
      onDeleted();
    } catch (err) {
      setError(String(err));
    }
  };

  const commitRename = async () => {
    const name = renameValue.trim();
    setRenaming(false);
    if (!name || name === workspace.display_name) return;
    try {
      await ipc.workspaceUpsert({ ...workspace, display_name: name });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div>
      {/* Header row — title + on/off switch + destructive action.
          The switch sits inline with the title so the workspace's state
          is always above the fold. */}
      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          gap: 16,
          marginBottom: 4,
        }}
      >
        <div style={{ flex: 1, minWidth: 0 }}>
          {renaming ? (
            <Input
              autoFocus
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onBlur={commitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitRename();
                if (e.key === "Escape") setRenaming(false);
              }}
              className="h-8 max-w-[360px] text-lg font-semibold"
            />
          ) : (
            <h2
              className="mac-h2"
              onClick={() => {
                setRenameValue(workspace.display_name);
                setRenaming(true);
              }}
              title="Click to rename"
              style={{ cursor: "text" }}
            >
              {workspace.display_name}
              <Pencil
                size={13}
                strokeWidth={1.8}
                style={{
                  display: "inline-block",
                  marginLeft: 8,
                  color: "var(--mac-fg-tertiary)",
                  verticalAlign: "middle",
                }}
              />
            </h2>
          )}
          <p className="mac-subtitle" style={{ marginBottom: 0 }}>
            {workspace.description ||
              "Group of rules that can be toggled together."}
          </p>
        </div>
        <button
          type="button"
          className={cn("mac-switch accent", workspace.enabled && "on")}
          aria-pressed={workspace.enabled}
          onClick={toggle}
          title={workspace.enabled ? "Disable workspace" : "Enable workspace"}
        />
        <button
          type="button"
          className="mac-tbtn"
          onClick={remove}
          title="Delete workspace (rules become ungrouped)"
          style={{ color: "var(--mac-danger)" }}
        >
          <Trash2 size={13} strokeWidth={1.8} />
        </button>
      </div>

      <div style={{ height: 16 }} />

      {/* Stat grid */}
      <div className="mac-stat-grid">
        <div className="mac-stat">
          <div className="mac-stat-label">Status</div>
          <div className="mac-stat-value" style={{ fontSize: 18 }}>
            {workspace.enabled ? "Active" : "Paused"}
          </div>
          <div
            className="mac-stat-trend"
            style={{
              color: workspace.enabled
                ? "var(--mac-ok)"
                : "var(--mac-fg-muted)",
            }}
          >
            {workspace.enabled
              ? "Rules participate in routing"
              : "Router skips these rules"}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">Rules</div>
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
              / {rules.length}
            </span>
          </div>
          <div className="mac-stat-trend">
            {rules.length === 0
              ? "No rules yet"
              : enabledRules === rules.length
              ? "All enabled"
              : `${rules.length - enabledRules} disabled`}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">Recent hits</div>
          <div className="mac-stat-value">{hitRecords.length}</div>
          <div className="mac-stat-trend">
            {hitRecords.length > 0
              ? `Last ${history.length} routes scanned`
              : "Awaiting traffic"}
          </div>
        </div>
      </div>

      {/* Rules in this workspace */}
      <div
        className="mac-card-title"
        style={{ display: "flex", alignItems: "center" }}
      >
        <span>Rules · {rules.length}</span>
        <span style={{ flex: 1 }} />
        <button
          type="button"
          className="mac-tbtn"
          onClick={() => onOpenRulesFiltered(workspace.id)}
          style={{
            height: 22,
            padding: "0 8px",
            fontSize: 11,
            color: "var(--mac-accent)",
            borderColor: "var(--mac-border-soft)",
            textTransform: "none",
            letterSpacing: 0,
          }}
        >
          Edit in Rules
          <ArrowRight size={11} strokeWidth={2} />
        </button>
      </div>
      <div className="mac-card">
        {rules.length === 0 ? (
          <div
            className="mac-row mac-muted"
            style={{ justifyContent: "center", padding: "24px 18px" }}
          >
            No rules in this workspace yet. Assign a rule's
            <span className="mac-mono" style={{ margin: "0 4px" }}>
              workspace_id
            </span>
            from the Rules tab.
          </div>
        ) : (
          // List order in `doc.rules` IS priority; preserve it here.
          rules.map((r) => {
            const position = doc.rules.findIndex((x) => x.id === r.id) + 1;
            return (
              <RuleRow
                key={r.id}
                rule={r}
                position={position}
                hits={countHits(r.id, history)}
              />
            );
          })
        )}
      </div>

      {/* Recent activity attributed to this workspace */}
      {hitRecords.length > 0 && (
        <>
          <div className="mac-card-title">Recent activity · {hitRecords.length}</div>
          <div className="mac-card">
            {hitRecords.slice(0, 10).map((rec, i) => (
              <ActivityRow key={i} record={rec} rule={ruleById(rec, doc)} />
            ))}
          </div>
        </>
      )}

      {error && (
        <div className="mac-card">
          <div className="mac-row">
            <span className="mac-tag danger">error</span>
            <span className="grow mac-muted">{error}</span>
          </div>
        </div>
      )}
    </div>
  );
}

function countHits(ruleId: string, history: RouteRecord[]): number {
  let n = 0;
  for (const r of history) if (r.matched_rule === ruleId) n++;
  return n;
}

function ruleById(rec: RouteRecord, doc: ConfigDocument): Rule | undefined {
  return rec.matched_rule
    ? doc.rules.find((r) => r.id === rec.matched_rule)
    : undefined;
}

function RuleRow({
  rule,
  position,
  hits,
}: {
  rule: Rule;
  position: number;
  hits: number;
}) {
  return (
    <div className="mac-row">
      <span
        className="mac-muted"
        style={{
          width: 30,
          fontSize: 12,
          fontVariantNumeric: "tabular-nums",
          textAlign: "right",
        }}
        title="Priority position — top of the global rules list wins."
      >
        #{position}
      </span>
      <div className="grow" style={{ minWidth: 0 }}>
        <div style={{ fontSize: 13 }}>
          {rule.note || (
            <span className="mac-muted">
              (no note · rule id {rule.id.slice(0, 8)})
            </span>
          )}
        </div>
        <div className="mac-mono mac-muted" style={{ fontSize: 11, marginTop: 2 }}>
          {describeWhenInline(rule)}
        </div>
      </div>
      <span
        className={`mac-tag ${rule.enabled ? "ok" : "neutral"}`}
        title={rule.enabled ? "Enabled" : "Disabled"}
      >
        {rule.enabled ? "on" : "off"}
      </span>
      {hits > 0 && (
        <span
          className="mac-mono mac-muted"
          style={{ fontSize: 11, fontVariantNumeric: "tabular-nums" }}
        >
          {hits} hit{hits === 1 ? "" : "s"}
        </span>
      )}
    </div>
  );
}

function ActivityRow({
  record,
  rule,
}: {
  record: RouteRecord;
  rule?: Rule;
}) {
  return (
    <div className="mac-row">
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
          fontSize: 12,
        }}
      >
        {record.context.url}
      </span>
      {rule?.note && (
        <span
          className="mac-muted"
          style={{
            fontSize: 11,
            maxWidth: 180,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {rule.note}
        </span>
      )}
      {record.decision.action === "open" && (
        <BrowserBadge
          browserId={record.decision.target.browser}
          profile={record.decision.target.profile}
          className="text-xs"
        />
      )}
    </div>
  );
}

function timeAgo(ms: number): string {
  const s = Math.round((Date.now() - ms) / 1000);
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return `${Math.round(s / 86400)}d ago`;
}

function describeWhenInline(rule: Rule): string {
  return describeMatcher(rule.when);
}

function describeMatcher(m: Rule["when"]): string {
  switch (m.op) {
    case "always":
      return "always";
    case "url-host":
      return `host ${m.pattern}`;
    case "url-path":
      return `path ${m.pattern}`;
    case "source-app":
      return `from ${m.name}`;
    case "source-browser":
      return `from browser ${m.browser}`;
    case "source-profile":
      return `from profile ${m.profile}`;
    case "all":
      return m.of.map(describeMatcher).join(" and ");
    case "any":
      return m.of.map(describeMatcher).join(" or ");
    case "not":
      return `not (${describeMatcher(m.of)})`;
  }
}
