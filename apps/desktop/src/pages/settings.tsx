import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TargetEditor } from "@/components/TargetEditor";
import { ipc } from "@/lib/ipc";
import { useTheme, type ThemeMode } from "@/lib/theme";
import type {
  BrowserTarget,
  ConfigDocument,
  InstalledBrowser,
  SetDefaultOutcome,
} from "@/lib/types";

interface Props {
  configEpoch: number;
}

export function SettingsPage({ configEpoch }: Props) {
  const [doc, setDoc] = useState<ConfigDocument | null>(null);
  const [configPath, setConfigPath] = useState<string | null>(null);
  const [isDefault, setIsDefault] = useState<boolean | null>(null);
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [importPath, setImportPath] = useState("");
  const [exportPath, setExportPath] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { mode: themeMode, active: themeActive, setMode: setThemeMode } =
    useTheme();

  const refresh = useCallback(async () => {
    try {
      const [d, c, b] = await Promise.all([
        ipc.doctor(),
        ipc.configGet(),
        ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
      ]);
      setConfigPath(d.config_path ?? null);
      setIsDefault(d.is_default_browser);
      setDoc(c);
      setBrowsers(b);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh, configEpoch]);

  const updateDefaultTarget = async (next: BrowserTarget) => {
    if (!doc) return;
    setError(null);
    try {
      await ipc.configReplace({ ...doc, default_target: next });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const setAsDefault = async () => {
    setError(null);
    setMessage(null);
    try {
      const outcome: SetDefaultOutcome = await ipc.requestSetDefaultBrowser();
      if (outcome.kind === "done") {
        setMessage("Set successfully. macOS may show a confirmation dialog.");
      } else if (outcome.kind === "user-consent-required") {
        setMessage(
          outcome.instructions_url
            ? `Open ${outcome.instructions_url} to finish setting LinkPilot as default.`
            : "Please finish the default-browser switch in System Settings.",
        );
      } else {
        setMessage("This platform doesn't expose a programmatic 'set default'.");
      }
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const toggleLaunchAtLogin = async (next: boolean) => {
    if (!doc) return;
    try {
      await ipc.configReplace({
        ...doc,
        settings: { ...doc.settings, launch_at_login: next },
      });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const doImport = async () => {
    setError(null);
    setMessage(null);
    try {
      await ipc.importConfig(importPath);
      setMessage(`Imported ${importPath}`);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const doExport = async () => {
    setError(null);
    setMessage(null);
    try {
      await ipc.exportConfig(exportPath);
      setMessage(`Exported to ${exportPath}`);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="space-y-4">
      <header>
        <h2 className="text-xl font-semibold tracking-tight">Settings</h2>
        <p className="text-sm text-muted-foreground">
          Default browser, autostart, appearance, and config IO.
        </p>
      </header>

      <Card>
        <CardHeader>
          <CardTitle>Default browser</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm">
              LinkPilot is currently the system default browser
            </span>
            <Badge variant={isDefault ? "success" : "destructive"}>
              {isDefault === null ? "…" : isDefault ? "yes" : "no"}
            </Badge>
          </div>
          <div className="flex items-center justify-between gap-3">
            <span className="text-xs text-muted-foreground">
              macOS will prompt to confirm. On Windows you'll be sent to the
              Settings → Default apps page.
            </span>
            <Button onClick={setAsDefault} className="shrink-0">
              Set LinkPilot as default
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Default target</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="mb-3 text-xs text-muted-foreground">
            Where to open links when <strong>no rule matches</strong>. Changing
            this rewrites the config file.
          </p>
          {doc ? (
            <div className="flex items-center gap-2">
              <TargetEditor
                value={doc.default_target}
                browsers={browsers}
                onChange={updateDefaultTarget}
              />
            </div>
          ) : (
            <span className="text-sm text-muted-foreground">Loading…</span>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>General</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm">Launch at login</span>
            <Checkbox
              checked={doc?.settings.launch_at_login ?? false}
              onCheckedChange={(v) => toggleLaunchAtLogin(v === true)}
            />
          </div>
          <div className="flex items-center justify-between gap-3">
            <span className="text-sm">Config file</span>
            <span className="font-mono text-xs text-muted-foreground">
              {configPath ?? "…"}
            </span>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <span className="text-sm">
              Theme
              {themeMode === "system" && (
                <span className="ml-1 text-xs text-muted-foreground">
                  — currently <span className="font-mono">{themeActive}</span>
                </span>
              )}
            </span>
            <Select
              value={themeMode}
              onValueChange={(v) => setThemeMode(v as ThemeMode)}
            >
              <SelectTrigger className="w-[160px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="system">System</SelectItem>
                <SelectItem value="light">Light</SelectItem>
                <SelectItem value="dark">Dark</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Import / Export</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="import-path">Import config from path</Label>
            <div className="flex gap-2">
              <Input
                id="import-path"
                value={importPath}
                placeholder="/absolute/path/to/some.json"
                onChange={(e) => setImportPath(e.target.value)}
              />
              <Button
                variant="outline"
                onClick={doImport}
                disabled={!importPath}
              >
                Import
              </Button>
            </div>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="export-path">Export config to path</Label>
            <div className="flex gap-2">
              <Input
                id="export-path"
                value={exportPath}
                placeholder="/absolute/path/to/save.json"
                onChange={(e) => setExportPath(e.target.value)}
              />
              <Button
                variant="outline"
                onClick={doExport}
                disabled={!exportPath}
              >
                Export
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {message && (
        <Card>
          <CardContent className="flex items-center gap-2 pt-4">
            <Badge variant="success">info</Badge>
            <span className="text-sm text-muted-foreground">{message}</span>
          </CardContent>
        </Card>
      )}
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
