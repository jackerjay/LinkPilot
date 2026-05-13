import { useEffect, useState } from "react";
import {
  Compass,
  FlaskConical,
  LayoutDashboard,
  ScrollText,
  Settings as SettingsIcon,
  Workflow,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { MenuBarPage } from "@/pages/menu-bar";
import { RulesPage } from "@/pages/rules";
import { InspectorPage } from "@/pages/inspector";
import { TestUrlPage } from "@/pages/test-url";
import { BrowsersPage } from "@/pages/browsers";
import { SettingsPage } from "@/pages/settings";
import { onConfigChanged } from "@/lib/ipc";
import { cn } from "@/lib/utils";
// 128×128 downscaled from docs/brand/icon.png (the master is the Tauri
// bundle source — too large to ship in the renderer JS bundle just for
// the 22pt sidebar logo). Regenerate with:
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
  label: string;
  icon: LucideIcon;
}

const TABS: Tab[] = [
  { id: "menu-bar", label: "Overview", icon: LayoutDashboard },
  { id: "rules", label: "Rules", icon: Workflow },
  { id: "test-url", label: "Test URL", icon: FlaskConical },
  { id: "inspector", label: "Inspector", icon: ScrollText },
  { id: "browsers", label: "Browsers", icon: Compass },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

export default function App() {
  const [tab, setTab] = useState<TabId>("menu-bar");
  const [configEpoch, setConfigEpoch] = useState(0);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onConfigChanged(() => setConfigEpoch((n) => n + 1)).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <TooltipProvider delayDuration={200}>
      <div className="relative grid h-screen grid-cols-[200px_1fr]">
        {/* Full-width title-bar drag region. Uses Tauri 2's
            `data-tauri-drag-region` attribute (more reliable than the
            -webkit-app-region CSS in WKWebView): a mousedown on this
            element calls window.startDragging() via Tauri's injected
            script. Sits above the sidebar + main columns with z-40 so
            it overlays the empty top strip on BOTH sides, including the
            area at the same y as the traffic lights but to their right.
            Traffic lights are rendered by macOS above the webview, so
            they keep working. */}
        <div
          data-tauri-drag-region
          className="absolute inset-x-0 top-0 z-40 h-14"
          aria-hidden
        />

        <aside className="flex flex-col gap-0.5 border-r border-sidebar-border bg-sidebar px-2.5 pb-3 pt-14 text-sidebar-foreground">
          <div className="flex items-center gap-2 px-2 pb-4">
            <img
              src={brandIcon}
              alt="LinkPilot"
              className="h-[22px] w-[22px] flex-shrink-0 rounded"
            />
            <h1 className="text-sm font-semibold tracking-tight">LinkPilot</h1>
          </div>
          {TABS.map((t) => {
            const Icon = t.icon;
            const isActive = tab === t.id;
            return (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className={cn(
                  "flex h-8 items-center gap-2.5 rounded-md px-2.5 text-left text-sm transition-colors",
                  isActive
                    ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                    : "text-sidebar-foreground hover:bg-accent hover:text-accent-foreground",
                )}
              >
                <Icon className="h-4 w-4 shrink-0" />
                {t.label}
              </button>
            );
          })}
        </aside>
        <main className="pt-12 pb-4 overflow-hidden">
          {/* The scrollable region is the INNER container, not <main>.
              That confines the scrollbar to the vertical space between
              <main>'s top/bottom padding instead of running edge-to-edge
              under the title-bar drag strip. Custom thin-scrollbar
              styling lives in styles/app.css (.scroll-shy). */}
          <div className="scroll-shy h-full overflow-y-auto overflow-x-hidden overscroll-contain px-10 pb-4">
            {tab === "menu-bar" && <MenuBarPage configEpoch={configEpoch} />}
            {tab === "rules" && <RulesPage configEpoch={configEpoch} />}
            {tab === "test-url" && <TestUrlPage configEpoch={configEpoch} />}
            {tab === "inspector" && <InspectorPage />}
            {tab === "browsers" && <BrowsersPage />}
            {tab === "settings" && <SettingsPage configEpoch={configEpoch} />}
          </div>
        </main>
      </div>
    </TooltipProvider>
  );
}
