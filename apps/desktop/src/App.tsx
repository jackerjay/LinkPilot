import { useEffect, useState } from "react";
import { MenuBarPage } from "./pages/menu-bar";
import { RulesPage } from "./pages/rules";
import { InspectorPage } from "./pages/inspector";
import { TestUrlPage } from "./pages/test-url";
import { BrowsersPage } from "./pages/browsers";
import { SettingsPage } from "./pages/settings";
import { onConfigChanged } from "./lib/ipc";

type TabId =
  | "menu-bar"
  | "rules"
  | "test-url"
  | "inspector"
  | "browsers"
  | "settings";

const TABS: { id: TabId; label: string }[] = [
  { id: "menu-bar", label: "Overview" },
  { id: "rules", label: "Rules" },
  { id: "test-url", label: "Test URL" },
  { id: "inspector", label: "Inspector" },
  { id: "browsers", label: "Browsers" },
  { id: "settings", label: "Settings" },
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
        <h1>LinkPilot</h1>
        {TABS.map((t) => (
          <button
            key={t.id}
            className={tab === t.id ? "active" : ""}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
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
