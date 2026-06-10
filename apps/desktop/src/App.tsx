import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import {
  Compass,
  FlaskConical,
  Gauge,
  ScrollText,
  Settings as SettingsIcon,
  Workflow,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { applyLanguage } from "@/i18n";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  OnboardingFlow,
  isOnboardingNeeded,
} from "@/onboarding/OnboardingFlow";
import { MenuBarPage } from "@/pages/menu-bar";
import { RulesPage } from "@/pages/rules";
import { InspectorPage } from "@/pages/inspector";
import { TestUrlPage } from "@/pages/test-url";
import { BrowsersPage } from "@/pages/browsers";
import { SettingsPage } from "@/pages/settings";
import { WorkspacePage } from "@/pages/workspace";
import { ipc, onConfigChanged, onRouteLogged } from "@/lib/ipc";
import { checkForUpdates, type UpdateCheckState } from "@/lib/update";
import type {
  ConfigDocument,
  DoctorReport,
  RouteRecord,
} from "@/lib/types";
import { cn } from "@/lib/utils";
// 128×128 downscaled from docs/brand/icon.png. Regenerate with:
//   sips -Z 128 docs/brand/icon.png --out apps/desktop/src/assets/brand.png
import brandIcon from "@/assets/brand.png";

type TabId =
  | "menu-bar"
  | "rules"
  | "test-url"
  | "inspector"
  | "browsers"
  | "settings";

interface Tab {
  id: TabId;
  /** i18n key under the `app.tabs` namespace. Resolved inside the component
   *  so a language switch re-renders the sidebar without a remount. */
  labelKey:
    | "overview"
    | "rules"
    | "testUrl"
    | "inspector"
    | "browsers"
    | "settings";
  icon: LucideIcon;
  /** Icon-chip fill, System-Settings style: a top-lit vertical gradient
   *  per destination. Brand indigo leads; the rest follow the macOS
   *  system palette. */
  tint: string;
}

const TABS: Tab[] = [
  {
    id: "menu-bar",
    labelKey: "overview",
    icon: Gauge,
    tint: "linear-gradient(180deg, #6a71f0, #5057e8)",
  },
  {
    id: "rules",
    labelKey: "rules",
    icon: Workflow,
    tint: "linear-gradient(180deg, #4cd964, #2bb14c)",
  },
  {
    id: "test-url",
    labelKey: "testUrl",
    icon: FlaskConical,
    tint: "linear-gradient(180deg, #ffb340, #f59500)",
  },
  {
    id: "inspector",
    labelKey: "inspector",
    icon: ScrollText,
    tint: "linear-gradient(180deg, #5ac8fa, #2da9e0)",
  },
  {
    id: "browsers",
    labelKey: "browsers",
    icon: Compass,
    tint: "linear-gradient(180deg, #0a84ff, #0066d6)",
  },
  {
    id: "settings",
    labelKey: "settings",
    icon: SettingsIcon,
    tint: "linear-gradient(180deg, #9a9aa0, #7c7c82)",
  },
];

function isUpdateActionable(state: UpdateCheckState): boolean {
  return (
    state.status === "downloading" ||
    state.status === "downloaded" ||
    (state.status === "error" && !!state.result?.available)
  );
}

export default function App() {
  const { t } = useTranslation("app");
  const [tab, setTab] = useState<TabId>("menu-bar");
  const [configEpoch, setConfigEpoch] = useState(0);
  const [config, setConfig] = useState<ConfigDocument | null>(null);
  const [doctor, setDoctor] = useState<DoctorReport | null>(null);
  const [recentRoutes, setRecentRoutes] = useState<RouteRecord[]>([]);
  const [updateCheck, setUpdateCheck] = useState<UpdateCheckState>({
    status: "idle",
  });
  const autoUpdateCheckStartedRef = useRef(false);
  // When non-null, the main content area renders the workspace detail
  // page for this id instead of whatever `tab` is. Sidebar workspace
  // clicks set this; sidebar tab clicks clear it.
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<
    string | null
  >(null);
  // Optional seed for Rules' filter. Set when WorkspacePage's "Edit in
  // Rules" jumps over — bumping `token` re-applies even when the user
  // had already toggled back to "all" in Rules in the meantime.
  const [pendingRulesFilter, setPendingRulesFilter] = useState<{
    filter: string;
    token: number;
  } | null>(null);
  // First-run gate. localStorage flag — cheap, no Rust IPC needed, and
  // resetting it (DevTools or `localStorage.clear()`) is the documented
  // way to re-run onboarding during testing.
  const [showOnboarding, setShowOnboarding] = useState(() =>
    isOnboardingNeeded(),
  );

  const refreshSidebarData = useCallback(async () => {
    try {
      const [cfg, doc, history] = await Promise.all([
        ipc.configGet(),
        ipc.doctor(),
        // Route history powers the per-workspace hit-count chips. 200
        // is plenty for a "today" sense without paginating; older
        // records get pushed out naturally by onRouteLogged streaming.
        ipc.routeHistory(200).catch(() => [] as RouteRecord[]),
      ]);
      setConfig(cfg);
      setDoctor(doc);
      setRecentRoutes(history);
    } catch (err) {
      // Sidebar metadata is non-critical chrome — render gracefully.
      console.error("sidebar refresh failed", err);
    }
  }, []);

  useEffect(() => {
    refreshSidebarData();
  }, [refreshSidebarData, configEpoch]);

  const runUpdateCheck = useCallback(async () => {
    setUpdateCheck({ status: "checking" });
    try {
      const currentVersion = await getVersion();
      const result = await checkForUpdates(currentVersion);
      if (!result.available) {
        setUpdateCheck({ status: "up-to-date", result });
        return;
      }

      if (!result.asset.sha256) {
        // Refuse to auto-download an unverified DMG. Releases ship a
        // `checksums.txt`; if it's missing or doesn't list our asset,
        // the user can still grab the installer manually from the
        // release page — but we never write an unverified binary to
        // the updates dir on their behalf.
        setUpdateCheck({
          status: "error",
          error: t("updates.checksumMissing"),
          checkedAt: Date.now(),
          result,
        });
        return;
      }
      setUpdateCheck({ status: "downloading", result });
      try {
        const download = await ipc.updateDownload({
          url: result.asset.downloadUrl,
          version: result.latestVersion,
          asset_name: result.asset.name,
          expected_bytes: result.asset.size,
          expected_sha256: result.asset.sha256,
        });
        setUpdateCheck({ status: "downloaded", result, download });
      } catch (downloadErr) {
        const error =
          downloadErr instanceof Error ? downloadErr.message : String(downloadErr);
        setUpdateCheck({
          status: "error",
          error,
          checkedAt: Date.now(),
          result,
        });
      }
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      setUpdateCheck({ status: "error", error, checkedAt: Date.now() });
    }
  }, []);

  useEffect(() => {
    if (showOnboarding) return;
    if (!config?.settings.auto_check_updates) {
      autoUpdateCheckStartedRef.current = false;
      return;
    }
    if (autoUpdateCheckStartedRef.current) return;
    autoUpdateCheckStartedRef.current = true;
    void runUpdateCheck();
  }, [config?.settings.auto_check_updates, runUpdateCheck, showOnboarding]);

  // Reconcile i18next with the persisted language preference. Re-runs
  // whenever ConfigDocument is reloaded (fsnotify echo, Settings save,
  // import / undo), so flipping the language from the CLI or another
  // window propagates here without a full app restart.
  useEffect(() => {
    if (!config) return;
    applyLanguage(config.settings.language);
  }, [config?.settings.language, config]);

  useEffect(() => {
    let unlistenConfig: (() => void) | undefined;
    let unlistenRoute: (() => void) | undefined;
    let unlistenNav: (() => void) | undefined;
    onConfigChanged(() => setConfigEpoch((n) => n + 1)).then((fn) => {
      unlistenConfig = fn;
    });
    onRouteLogged((record) => {
      // Prepend + cap; matches what menu-bar and inspector pages do so
      // the sidebar workspace hit counts stay in sync with what those
      // pages render.
      setRecentRoutes((prev) => [record, ...prev].slice(0, 200));
    }).then((fn) => {
      unlistenRoute = fn;
    });
    // Tray popover footer buttons deep-link via `tray:navigate` —
    // payload is the TabId string. Validate before applying so a
    // future event with a bogus tab can't put us into an invalid
    // state.
    listen<string>("tray:navigate", (event) => {
      const valid: TabId[] = [
        "menu-bar",
        "rules",
        "test-url",
        "inspector",
        "browsers",
        "settings",
      ];
      if ((valid as string[]).includes(event.payload)) {
        setSelectedWorkspaceId(null);
        setTab(event.payload as TabId);
      }
    }).then((fn) => {
      unlistenNav = fn;
    });
    return () => {
      unlistenConfig?.();
      unlistenRoute?.();
      unlistenNav?.();
    };
  }, []);

  const workspaces = config?.workspaces ?? [];
  const daemonVersion = doctor?.daemon_version?.replace(
    /^linkpilot-daemon\s+/,
    "v",
  );

  // Per-workspace hit count: walk recent routes, map each matched_rule
  // back to its workspace via the live config. Recomputed on every
  // render — cheap (O(rules + routes)) and avoids stale memoization.
  const ruleToWorkspace = new Map<string, string | null>();
  for (const r of config?.rules ?? []) {
    ruleToWorkspace.set(r.id, r.workspace_id ?? null);
  }
  const workspaceHits = new Map<string, number>();
  for (const rec of recentRoutes) {
    if (!rec.matched_rule) continue;
    const wsId = ruleToWorkspace.get(rec.matched_rule);
    if (!wsId) continue;
    workspaceHits.set(wsId, (workspaceHits.get(wsId) ?? 0) + 1);
  }

  const openWorkspaceDetail = (workspaceId: string) => {
    setSelectedWorkspaceId(workspaceId);
  };

  const goToTab = (next: TabId) => {
    setSelectedWorkspaceId(null);
    setTab(next);
  };

  const openRulesFilteredToWorkspace = (workspaceId: string) => {
    setPendingRulesFilter({ filter: workspaceId, token: Date.now() });
    setSelectedWorkspaceId(null);
    setTab("rules");
  };

  if (showOnboarding) {
    return (
      <TooltipProvider delayDuration={200}>
        <OnboardingFlow
          onFinish={() => {
            setShowOnboarding(false);
            // Refresh sidebar / page data — onboarding may have written
            // rules that the rest of the UI needs to see immediately.
            setConfigEpoch((n) => n + 1);
          }}
        />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider delayDuration={200}>
      <div
        className="mac-window-bg relative grid h-screen"
        style={{ gridTemplateColumns: "220px 1fr" }}
      >
        {/* Full-width title-bar drag region. Tauri 2's
            data-tauri-drag-region attribute calls window.startDragging() on
            mousedown. Sits above sidebar+main with z-40 so the empty top
            strip on both sides is draggable. Traffic lights are rendered
            by macOS above the webview and keep working. */}
        <div
          data-tauri-drag-region
          className="absolute inset-x-0 top-0 z-40 h-12"
          aria-hidden
        />

        <aside className="mac-sidebar">
          {/* Top padding (clearing the macOS traffic-light dots) lives in
              `.mac-sidebar` itself — applying `pt-9` here is overridden
              by the unlayered class rule and silently loses. */}
          {/* Pilot wordmark — sits below the traffic-light row so the
              22pt brand mark + "Pilot" text occupy the same y as the
              native window title would. */}
          {/* Brand lockup — mirrors the website nav: glowing mark +
              wordmark over a small-caps role line, so the in-app brand
              matches the marketing surface. */}
          <div className="flex items-center gap-3 px-2 pb-4 pt-1">
            <img
              src={brandIcon}
              alt="LinkPilot"
              className="h-[36px] w-[36px] flex-shrink-0 rounded-[9px]"
              style={{
                boxShadow:
                  "0 2px 10px color-mix(in srgb, var(--mac-accent) 38%, transparent), 0 0 0 0.5px rgba(0, 0, 0, 0.06)",
              }}
            />
            <div style={{ lineHeight: 1.15 }}>
              <h1
                className="font-bold"
                style={{ fontSize: 16.5, letterSpacing: "-0.01em", margin: 0 }}
              >
                LinkPilot
              </h1>
              <div
                style={{
                  fontSize: 9,
                  fontWeight: 600,
                  letterSpacing: "0.09em",
                  textTransform: "uppercase",
                  color: "var(--mac-fg-muted)",
                  marginTop: 2,
                }}
              >
                macOS Link Router
              </div>
            </div>
          </div>

          {TABS.map((tabDef) => {
            const Icon = tabDef.icon;
            // A tab is visually active only when no workspace detail
            // page is taking over the right pane.
            const isActive =
              tab === tabDef.id && selectedWorkspaceId === null;
            return (
              <button
                key={tabDef.id}
                onClick={() => goToTab(tabDef.id)}
                className={cn("mac-sidebar-item", isActive && "active")}
              >
                <span
                  className="mac-sidebar-chip"
                  style={{ background: tabDef.tint }}
                  aria-hidden
                >
                  <Icon size={15} strokeWidth={1.8} />
                </span>
                <span>{t(`tabs.${tabDef.labelKey}`)}</span>
                {tabDef.id === "settings" &&
                  isUpdateActionable(updateCheck) && (
                    <span
                      className="mac-tag"
                      style={{ marginLeft: "auto", fontSize: 10 }}
                    >
                      {t("updates.tag")}
                    </span>
                  )}
              </button>
            );
          })}

          {workspaces.length > 0 && (
            <>
              <div className="mac-sidebar-section">
                {t("sidebar.workspaces")}
              </div>
              {workspaces.map((w) => {
                const hits = workspaceHits.get(w.id) ?? 0;
                const isActive = selectedWorkspaceId === w.id;
                return (
                  <button
                    key={w.id}
                    type="button"
                    onClick={() => openWorkspaceDetail(w.id)}
                    title={t("sidebar.workspaceOpenTitle", {
                      name: w.display_name,
                    })}
                    className={cn(
                      "mac-sidebar-item",
                      isActive && "active",
                      !w.enabled && "opacity-60",
                    )}
                  >
                    <span
                      className="mac-symbol-icon inline-flex h-[15px] w-[15px] items-center justify-center"
                      aria-hidden
                    >
                      {/* Dot reflects the workspace's `enabled` flag —
                          green for on (rules in this workspace
                          participate in routing), grey for off (router
                          skips them). Matches the on/off mental model
                          of the WorkspacesCard switch. */}
                      <span
                        className="inline-block h-[9px] w-[9px] rounded-full"
                        style={{
                          background: w.enabled
                            ? "var(--mac-ok)"
                            : "var(--mac-fg-tertiary)",
                        }}
                      />
                    </span>
                    <span className="truncate flex-1 text-left">
                      {w.display_name}
                    </span>
                    {/* Hit-count chip — shows the number of routes in the
                        last 200 history records that fired a rule owned
                        by this workspace. Hidden at 0 to keep the
                        sidebar quiet for first-run / unused workspaces. */}
                    {hits > 0 && (
                      <span
                        className="mac-mono"
                        style={{
                          fontSize: 10.5,
                          color: isActive
                            ? "rgba(255, 255, 255, 0.8)"
                            : "var(--mac-fg-muted)",
                          fontVariantNumeric: "tabular-nums",
                          minWidth: 18,
                          textAlign: "right",
                        }}
                      >
                        {hits}
                      </span>
                    )}
                  </button>
                );
              })}
            </>
          )}

          <div className="mac-sidebar-spacer" />
          <div className="mac-sidebar-footer">
            <span
              className={cn(
                "mac-dot",
                doctor ? "ok" : "warn",
              )}
            />
            <span>
              {t("sidebar.daemon")}
              {daemonVersion ? ` · ${daemonVersion}` : ""}
            </span>
          </div>
        </aside>

        <main className="flex flex-col overflow-hidden">
          {/* Toolbar — height and top padding come from `.mac-toolbar`
              (64px tall, content bottom-aligned with 12px bottom
              padding) so the title clears the 48px drag-region strip
              above. */}
          <div className="mac-toolbar">
            <span className="mac-toolbar-title">
              {selectedWorkspaceId
                ? config?.workspaces.find(
                    (w) => w.id === selectedWorkspaceId,
                  )?.display_name ?? t("sidebar.workspaceFallbackTitle")
                : (() => {
                    const def = TABS.find((td) => td.id === tab);
                    return def ? t(`tabs.${def.labelKey}`) : "";
                  })()}
            </span>
          </div>
          <div className="mac-scroll">
            {selectedWorkspaceId ? (
              <WorkspacePage
                workspaceId={selectedWorkspaceId}
                configEpoch={configEpoch}
                onOpenRulesFiltered={openRulesFilteredToWorkspace}
                onDeleted={() => goToTab("rules")}
              />
            ) : (
              <>
                {tab === "menu-bar" && (
                  <MenuBarPage configEpoch={configEpoch} />
                )}
                {tab === "rules" && (
                  <RulesPage
                    configEpoch={configEpoch}
                    pendingFilter={pendingRulesFilter}
                  />
                )}
                {tab === "test-url" && (
                  <TestUrlPage configEpoch={configEpoch} />
                )}
                {tab === "inspector" && <InspectorPage />}
                {tab === "browsers" && <BrowsersPage />}
                {tab === "settings" && (
                  <SettingsPage
                    configEpoch={configEpoch}
                    updateCheck={updateCheck}
                    onCheckForUpdates={runUpdateCheck}
                  />
                )}
              </>
            )}
          </div>
        </main>
      </div>
    </TooltipProvider>
  );
}
