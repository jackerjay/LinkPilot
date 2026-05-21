// Menu-bar tray popover. 360×500 borderless Tauri window labelled
// "tray", anchored under the menu-bar icon. The Rust side
// (src-tauri/src/tray.rs) handles show/hide/positioning; this
// component renders the content and routes all mutations through the
// existing IPC so anything you flip here is immediately reflected in
// the main window (and vice versa).

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { invoke } from "@tauri-apps/api/core";
import {
  Clock,
  Settings as SettingsIcon,
  SquarePen,
  Workflow,
} from "lucide-react";
import { BrowserBadge } from "@/components/BrowserBadge";
import { applyLanguage } from "@/i18n";
import { ipc, onConfigChanged, onRouteLogged } from "@/lib/ipc";
import type {
  ConfigDocument,
  DoctorReport,
  RouteRecord,
  Workspace,
} from "@/lib/types";
import brandIcon from "@/assets/brand.png";

export function TrayPopover() {
  const { t } = useTranslation("tray");
  const [doctor, setDoctor] = useState<DoctorReport | null>(null);
  const [config, setConfig] = useState<ConfigDocument | null>(null);
  const [recent, setRecent] = useState<RouteRecord[]>([]);

  const refresh = useCallback(async () => {
    try {
      const [d, c, h] = await Promise.all([
        ipc.doctor(),
        ipc.configGet(),
        ipc.routeHistory(10),
      ]);
      setDoctor(d);
      setConfig(c);
      setRecent(h);
    } catch {
      /* Popover is best-effort chrome; fail silent. */
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Stream live route events so Recent ticks up while the popover is
  // open. Also subscribe to config-changed so flips made from the main
  // window (e.g. toggling a workspace from WorkspacesCard) propagate.
  useEffect(() => {
    let off1: (() => void) | undefined;
    let off2: (() => void) | undefined;
    onRouteLogged((record) => {
      setRecent((prev) => [record, ...prev].slice(0, 10));
    }).then((fn) => {
      off1 = fn;
    });
    onConfigChanged(() => {
      refresh();
    }).then((fn) => {
      off2 = fn;
    });
    return () => {
      off1?.();
      off2?.();
    };
  }, [refresh]);

  const workspaces: Workspace[] = config?.workspaces ?? [];
  const recentOpens = recent
    .filter((r) => r.decision.action === "open")
    .slice(0, 4);

  useEffect(() => {
    if (!config) return;
    applyLanguage(config.settings.language);
  }, [config?.settings.language, config]);

  // Each action shows the main window and (when a tab is provided)
  // deep-links the App's tab state via the `tray:navigate` event the
  // Rust side emits. Hides the popover in the same call atomically.
  const openMain = (tab?: "menu-bar" | "rules" | "inspector" | "settings") => {
    invoke("tray_open_main", { tab: tab ?? null }).catch(() => {});
  };

  const toggleWorkspace = async (ws: Workspace) => {
    try {
      await ipc.workspaceSetEnabled(ws.id, !ws.enabled);
    } catch (err) {
      console.error("tray: workspaceSetEnabled failed", err);
    }
  };

  return (
    <div className="mac-popover">
      <div className="mac-popover-arrow" />

      {/* Header */}
      <div
        style={{
          padding: "16px 16px 12px",
          display: "flex",
          alignItems: "center",
          gap: 10,
        }}
      >
        <img
          src={brandIcon}
          width={28}
          height={28}
          alt=""
          style={{
            borderRadius: 7,
            boxShadow:
              "0 0 0 0.5px rgba(0,0,0,0.08), 0 1px 3px rgba(0,0,0,0.12)",
          }}
        />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div
            style={{
              fontWeight: 600,
              fontSize: 13,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            LinkPilot
          </div>
          <div
            className="mac-muted"
            style={{
              fontSize: 11,
              display: "inline-flex",
              gap: 5,
              alignItems: "center",
              maxWidth: "100%",
              minWidth: 0,
              whiteSpace: "nowrap",
            }}
          >
            <span
              className={`mac-dot ${doctor ? "ok" : "warn"}`}
              style={{ width: 6, height: 6, flex: "0 0 6px" }}
            />
            <span
              style={{
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {t("status", { count: recent.length })}
            </span>
          </div>
        </div>
        <button
          type="button"
          className="mac-tbtn"
          style={{ height: 22, minWidth: 22, padding: 0 }}
          aria-label={t("openMain")}
          title={t("openMain")}
          onClick={() => openMain()}
        >
          <SquarePen size={12} strokeWidth={1.8} />
        </button>
      </div>

      {/* Workspaces — each pill toggles its workspace's enabled flag.
          Enabled = accent border + accent text; disabled = neutral. */}
      {workspaces.length > 0 && (
        <>
          <div
            style={{
              padding: "0 16px 8px",
              fontSize: 10,
              fontWeight: 600,
              letterSpacing: "0.06em",
              textTransform: "uppercase",
              color: "var(--mac-fg-muted)",
            }}
          >
            {t("workspaces")}
          </div>
          <div style={{ padding: "0 12px 10px" }}>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: `repeat(${Math.min(
                  workspaces.length,
                  3,
                )}, 1fr)`,
                gap: 6,
              }}
            >
              {workspaces.slice(0, 6).map((w) => (
                <button
                  key={w.id}
                  type="button"
                  className="mac-tbtn"
                  onClick={() => toggleWorkspace(w)}
                  title={t("toggleWorkspace", {
                    action: w.enabled ? t("disable") : t("enable"),
                    name: w.display_name,
                  })}
                  style={{
                    minWidth: 0,
                    justifyContent: "center",
                    gap: 6,
                    borderColor: w.enabled
                      ? "var(--mac-accent)"
                      : undefined,
                    color: w.enabled ? "var(--mac-accent)" : undefined,
                    fontWeight: w.enabled ? 600 : 400,
                  }}
                >
                  <span
                    className="mac-dot"
                    style={{
                      width: 7,
                      height: 7,
                      background: w.enabled
                        ? "var(--mac-ok)"
                        : "var(--mac-fg-tertiary)",
                      flex: "0 0 7px",
                    }}
                  />
                  <span
                    style={{
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {w.display_name}
                  </span>
                </button>
              ))}
            </div>
          </div>
        </>
      )}

      {/* Recent routes */}
      <div
        style={{
          padding: "8px 16px 4px",
          fontSize: 10,
          fontWeight: 600,
          letterSpacing: "0.06em",
          textTransform: "uppercase",
          color: "var(--mac-fg-muted)",
        }}
      >
        {t("recent")}
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: "0 8px" }}>
        {recentOpens.length === 0 ? (
          <div
            className="mac-muted"
            style={{ fontSize: 11.5, padding: "12px 10px", textAlign: "center" }}
          >
            {t("emptyRecent")}
          </div>
        ) : (
          recentOpens.map((r, i) => {
            if (r.decision.action !== "open") return null;
            const target = r.decision.target;
            return (
              <div
                key={i}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "8px 8px",
                  borderRadius: 7,
                }}
              >
                <BrowserBadge
                  browserId={target.browser}
                  profile={target.profile}
                  className="shrink-0"
                />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    className="mac-mono"
                    style={{
                      fontSize: 11.5,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {r.context.url.replace(/^https?:\/\//, "")}
                  </div>
                  <div
                    style={{
                      fontSize: 10.5,
                      color: "var(--mac-fg-muted)",
                      marginTop: 1,
                    }}
                  >
                    {formatTimeAgo(t, r.timestamp_ms)}
                    {target.profile ? ` · ${target.profile}` : ""}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Footer actions — each deep-links to a specific tab in the
          main window via the `tray:navigate` event Rust emits from
          `tray_open_main`. */}
      <div
        style={{
          display: "flex",
          padding: 8,
          gap: 4,
          borderTop: "0.5px solid var(--mac-divider)",
        }}
      >
        <button
          type="button"
          className="mac-tbtn"
          style={{ flex: 1, justifyContent: "center" }}
          onClick={() => openMain("rules")}
        >
          <Workflow size={13} strokeWidth={1.8} />
          <span>{t("footer.rules")}</span>
        </button>
        <button
          type="button"
          className="mac-tbtn"
          style={{ flex: 1, justifyContent: "center" }}
          onClick={() => openMain("inspector")}
        >
          <Clock size={13} strokeWidth={1.8} />
          <span>{t("footer.history")}</span>
        </button>
        <button
          type="button"
          className="mac-tbtn"
          style={{ flex: 1, justifyContent: "center" }}
          onClick={() => openMain("settings")}
        >
          <SettingsIcon size={13} strokeWidth={1.8} />
          <span>{t("footer.settings")}</span>
        </button>
      </div>
    </div>
  );
}

function formatTimeAgo(t: TFunction<"tray">, ms: number): string {
  const s = Math.round((Date.now() - ms) / 1000);
  if (s < 60) return t("timeAgo.seconds", { n: s });
  if (s < 3600) return t("timeAgo.minutes", { n: Math.round(s / 60) });
  if (s < 86400) return t("timeAgo.hours", { n: Math.round(s / 3600) });
  return t("timeAgo.days", { n: Math.round(s / 86400) });
}
