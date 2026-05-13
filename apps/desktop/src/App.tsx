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
      <div className="grid h-screen grid-cols-[200px_1fr]">
        {/* Sidebar — top padding clears the macOS traffic lights overlay.
            Whole sidebar is a drag region (set on the div via inline style
            since Tailwind has no utility for -webkit-app-region). Buttons
            opt out so clicks register. */}
        <aside
          className="flex flex-col gap-0.5 border-r border-sidebar-border bg-sidebar px-2.5 pb-3 pt-14 text-sidebar-foreground"
          style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
        >
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
                style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
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
        <main className="flex flex-col overflow-hidden">
          {/* Title-bar replacement: full-width drag region covering the
              entire empty space above the page heading. With
              `titleBarStyle: Overlay` macOS hides the title bar but does
              not auto-mark anything as draggable; sidebar drags from its
              own background, and this strip covers the right column.
              Sized to match the visible "header area" the user expects
              (~64px) so dragging works anywhere above the page heading. */}
          <div
            className="h-16 flex-shrink-0"
            style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
            aria-hidden
          />
          <div className="flex-1 overflow-y-auto overflow-x-hidden overscroll-contain px-10 pb-8">
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
