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
import { MenuBarPage } from "./pages/menu-bar";
import { RulesPage } from "./pages/rules";
import { InspectorPage } from "./pages/inspector";
import { TestUrlPage } from "./pages/test-url";
import { BrowsersPage } from "./pages/browsers";
import { SettingsPage } from "./pages/settings";
import { onConfigChanged } from "./lib/ipc";
// 128×128 downscaled from docs/brand/icon.png (the master 1254×1254 is the
// Tauri bundle source — too large to ship in the renderer JS bundle just
// for a 22pt sidebar logo). Regenerate with:
//   sips -Z 128 docs/brand/icon.png --out apps/desktop/src/assets/brand.png
import brandIcon from "./assets/brand.png";

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
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <img src={brandIcon} alt="LinkPilot" />
          <h1>LinkPilot</h1>
        </div>
        {TABS.map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              className={tab === t.id ? "active" : ""}
              onClick={() => setTab(t.id)}
            >
              <Icon />
              {t.label}
            </button>
          );
        })}
      </aside>
      <main className="content">
        {tab === "menu-bar" && <MenuBarPage configEpoch={configEpoch} />}
        {tab === "rules" && <RulesPage configEpoch={configEpoch} />}
        {tab === "test-url" && <TestUrlPage configEpoch={configEpoch} />}
        {tab === "inspector" && <InspectorPage />}
        {tab === "browsers" && <BrowsersPage />}
        {tab === "settings" && <SettingsPage configEpoch={configEpoch} />}
      </main>
    </div>
  );
}
