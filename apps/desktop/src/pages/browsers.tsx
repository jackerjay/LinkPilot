import { useEffect, useState } from "react";
import { AppIcon } from "@/components/AppIcon";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { appPathFromExecutable } from "@/lib/browsers";
import { ipc } from "@/lib/ipc";
import type { BrowserProfile, InstalledBrowser } from "@/lib/types";

interface Entry {
  browser: InstalledBrowser;
  profiles: BrowserProfile[];
  error?: string;
}

export function BrowsersPage() {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const installed = await ipc.listBrowsers();
        const out: Entry[] = await Promise.all(
          installed.map(async (b) => {
            try {
              return { browser: b, profiles: await ipc.listProfiles(b.id) };
            } catch (err) {
              return { browser: b, profiles: [], error: String(err) };
            }
          }),
        );
        setEntries(out);
      } catch (err) {
        setError(String(err));
      }
    })();
  }, []);

  if (error) {
    return (
      <div className="space-y-4">
        <header>
          <h2 className="text-xl font-semibold tracking-tight">Browsers</h2>
        </header>
        <Card>
          <CardContent className="flex items-center gap-2 pt-4">
            <Badge variant="destructive">error</Badge>
            <span className="text-sm text-muted-foreground">{error}</span>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <header>
        <h2 className="text-xl font-semibold tracking-tight">Browsers</h2>
        <p className="text-sm text-muted-foreground">
          Detected browsers and the profiles LinkPilot can route to.
        </p>
      </header>

      {entries === null ? (
        <Card>
          <CardContent className="p-8 text-center text-sm text-muted-foreground">
            Scanning…
          </CardContent>
        </Card>
      ) : entries.length === 0 ? (
        <Card>
          <CardContent className="p-8 text-center text-sm text-muted-foreground">
            No browsers detected.
          </CardContent>
        </Card>
      ) : (
        entries.map((e) => (
          <Card key={e.browser.id}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0">
              <div className="flex items-center gap-3">
                <AppIcon
                  bundleId={e.browser.platform_app_id ?? undefined}
                  appPath={appPathFromExecutable(e.browser.executable)}
                  size={32}
                  alt={e.browser.display_name}
                />
                <div>
                  <CardTitle>{e.browser.display_name}</CardTitle>
                  <span className="font-mono text-xs text-muted-foreground">
                    {e.browser.id}
                  </span>
                </div>
              </div>
              <Badge variant="secondary">{e.browser.kind}</Badge>
            </CardHeader>
            <CardContent className="space-y-2">
              <div className="select-text font-mono text-xs text-muted-foreground">
                {e.browser.executable}
              </div>
              {e.error && (
                <div className="flex items-center gap-2">
                  <Badge variant="destructive">profiles</Badge>
                  <span className="text-xs text-muted-foreground">
                    {e.error}
                  </span>
                </div>
              )}
              {e.profiles.length > 0 && (
                <div className="space-y-1 pt-2">
                  <div className="text-xs text-muted-foreground">Profiles</div>
                  <div className="divide-y divide-border">
                    {e.profiles.map((p) => (
                      <div
                        key={p.id}
                        className="flex items-center justify-between py-1.5"
                      >
                        <span className="text-sm">
                          {p.display_name}
                          {p.email && (
                            <span className="ml-2 text-xs text-muted-foreground">
                              · {p.email}
                            </span>
                          )}
                        </span>
                        <span className="font-mono text-xs text-muted-foreground">
                          {p.id}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        ))
      )}
    </div>
  );
}
