// Workspace detail page — opened when the user clicks a workspace in
// the sidebar. Self-contained surface (not a Rules tab pre-filter)
// because the user needs a place where the workspace itself is the
// subject: its name, on/off state, the rules it owns, and the routing
// traffic it has attracted.

import { useCallback, useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
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
  const { t } = useTranslation("workspace");
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
        <h2 className="mac-h2">{t("fallbackTitle")}</h2>
        <p className="mac-subtitle">{t("deletedSubtitle")}</p>
      </div>
    );
  }

  if (!workspace || !doc) {
    return (
      <div>
        <h2 className="mac-h2">{t("fallbackTitle")}</h2>
        <p className="mac-subtitle">{t("loading")}</p>
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
              title={t("renameTitle")}
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
              t("defaultDescription")}
          </p>
        </div>
        <button
          type="button"
          className={cn("mac-switch accent", workspace.enabled && "on")}
          aria-pressed={workspace.enabled}
          onClick={toggle}
          title={workspace.enabled ? t("disableTitle") : t("enableTitle")}
        />
        <button
          type="button"
          className="mac-tbtn"
          onClick={remove}
          title={t("deleteTitle")}
          style={{ color: "var(--mac-danger)" }}
        >
          <Trash2 size={13} strokeWidth={1.8} />
        </button>
      </div>

      <div style={{ height: 16 }} />

      {/* Stat grid */}
      <div className="mac-stat-grid">
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.status")}</div>
          <div className="mac-stat-value" style={{ fontSize: 18 }}>
            {workspace.enabled ? t("stats.active") : t("stats.paused")}
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
              ? t("stats.activeHint")
              : t("stats.pausedHint")}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.rules")}</div>
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
              ? t("stats.noRules")
              : enabledRules === rules.length
              ? t("stats.allEnabled")
              : t("stats.disabled", { count: rules.length - enabledRules })}
          </div>
        </div>
        <div className="mac-stat">
          <div className="mac-stat-label">{t("stats.recentHits")}</div>
          <div className="mac-stat-value">{hitRecords.length}</div>
          <div className="mac-stat-trend">
            {hitRecords.length > 0
              ? t("stats.lastScanned", { count: history.length })
              : t("stats.awaiting")}
          </div>
        </div>
      </div>

      {/* Rules in this workspace */}
      <div
        className="mac-card-title"
        style={{ display: "flex", alignItems: "center" }}
      >
        <span>{t("rules.card", { count: rules.length })}</span>
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
          {t("rules.edit")}
          <ArrowRight size={11} strokeWidth={2} />
        </button>
      </div>
      <div className="mac-card">
        {rules.length === 0 ? (
          <div
            className="mac-row mac-muted"
            style={{ justifyContent: "center", padding: "24px 18px" }}
          >
            <Trans
              i18nKey="rules.empty"
              ns="workspace"
              components={{
                code: <span className="mac-mono" style={{ margin: "0 4px" }} />,
              }}
            />
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
          <div className="mac-card-title">
            {t("activity.card", { count: hitRecords.length })}
          </div>
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
            <span className="mac-tag danger">{t("errorTag")}</span>
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
  const { t } = useTranslation("workspace");
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
        title={t("ruleRow.priorityTitle")}
      >
        #{position}
      </span>
      <div className="grow" style={{ minWidth: 0 }}>
        <div style={{ fontSize: 13 }}>
          {rule.note || (
            <span className="mac-muted">
              {t("ruleRow.noNote", { id: rule.id.slice(0, 8) })}
            </span>
          )}
        </div>
        <div className="mac-mono mac-muted" style={{ fontSize: 11, marginTop: 2 }}>
          {describeWhenInline(t, rule)}
        </div>
      </div>
      <span
        className={`mac-tag ${rule.enabled ? "ok" : "neutral"}`}
        title={rule.enabled ? t("ruleRow.enabledTitle") : t("ruleRow.disabledTitle")}
      >
        {rule.enabled ? t("ruleRow.on") : t("ruleRow.off")}
      </span>
      {hits > 0 && (
        <span
          className="mac-mono mac-muted"
          style={{ fontSize: 11, fontVariantNumeric: "tabular-nums" }}
        >
          {t("ruleRow.hit", { count: hits })}
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
  const { t } = useTranslation("workspace");
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
        {formatTimeAgo(t, record.timestamp_ms)}
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

function formatTimeAgo(t: TFunction<"workspace">, ms: number): string {
  const s = Math.round((Date.now() - ms) / 1000);
  if (s < 60) return t("timeAgo.seconds", { n: s });
  if (s < 3600) return t("timeAgo.minutes", { n: Math.round(s / 60) });
  if (s < 86400) return t("timeAgo.hours", { n: Math.round(s / 3600) });
  return t("timeAgo.days", { n: Math.round(s / 86400) });
}

function describeWhenInline(t: TFunction<"workspace">, rule: Rule): string {
  return describeMatcher(t, rule.when);
}

function describeMatcher(t: TFunction<"workspace">, m: Rule["when"]): string {
  switch (m.op) {
    case "always":
      return t("matcher.always");
    case "url-host":
      return t("matcher.host", { pattern: m.pattern });
    case "url-path":
      return t("matcher.path", { pattern: m.pattern });
    case "source-app":
      return t("matcher.from", { name: m.name });
    case "source-browser":
      return t("matcher.fromBrowser", { browser: m.browser });
    case "source-profile":
      return t("matcher.fromProfile", { profile: m.profile });
    case "all":
      return m.of.map((child) => describeMatcher(t, child)).join(t("matcher.and"));
    case "any":
      return m.of.map((child) => describeMatcher(t, child)).join(t("matcher.or"));
    case "not":
      return t("matcher.not", { value: describeMatcher(t, m.of) });
  }
}
