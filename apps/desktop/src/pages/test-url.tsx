// Dry-run a URL through the live routing engine. No browser is launched;
// the daemon evaluates the rules and returns the decision + the full
// MatcherEval tree, which we render exactly like the Inspector does.

import { useCallback, useEffect, useMemo, useState } from "react";
import { AppIcon } from "@/components/AppIcon";
import { AppPickerButton } from "@/components/AppPickerButton";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ExplanationView } from "@/components/Explanation";
import { ipc } from "@/lib/ipc";
import { appPathFromExecutable } from "@/lib/browsers";
import type {
  BrowserProfile,
  ConfigDocument,
  Explained,
  InstalledBrowser,
  Rule,
} from "@/lib/types";
import { DecisionLine } from "./menu-bar";

interface Props {
  configEpoch: number;
}

const NONE = "__none";

export function TestUrlPage({ configEpoch }: Props) {
  const [url, setUrl] = useState("https://github.com/anthropics/anthropic-cookbook");
  const [fromApp, setFromApp] = useState("");
  // Tracks the bundle id captured by AppPickerButton — kept in sync with
  // `fromApp` so manual typing (which can't know the bundle id) clears it,
  // and the routing backend then falls back to name matching. See
  // `routing::eval_matcher` for the matching rules.
  const [fromAppBundleId, setFromAppBundleId] = useState<string | null>(null);
  const [fromBrowser, setFromBrowser] = useState("");
  const [fromProfile, setFromProfile] = useState("");

  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [config, setConfig] = useState<ConfigDocument | null>(null);

  const [result, setResult] = useState<Explained | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      ipc.listBrowsers().catch(() => [] as InstalledBrowser[]),
      ipc.configGet(),
    ]).then(([b, c]) => {
      setBrowsers(b);
      setConfig(c);
    });
  }, [configEpoch]);

  useEffect(() => {
    if (!fromBrowser) {
      setProfiles([]);
      setFromProfile("");
      return;
    }
    let alive = true;
    ipc
      .listProfiles(fromBrowser)
      .then((p) => {
        if (alive) setProfiles(p);
      })
      .catch(() => {
        if (alive) setProfiles([]);
      });
    return () => {
      alive = false;
    };
  }, [fromBrowser]);

  const evaluate = useCallback(async () => {
    if (!url.trim()) {
      setResult(null);
      setError(null);
      return;
    }
    try {
      const out = await ipc.routeEvaluate({
        url: url.trim(),
        from_app: fromApp.trim() || null,
        from_app_bundle_id: fromAppBundleId || null,
        from_browser: fromBrowser || null,
        from_profile: fromProfile || null,
      });
      setResult(out);
      setError(null);
    } catch (e) {
      setResult(null);
      setError(String(e));
    }
  }, [url, fromApp, fromAppBundleId, fromBrowser, fromProfile]);

  useEffect(() => {
    const t = setTimeout(() => {
      evaluate().catch(console.error);
    }, 250);
    return () => clearTimeout(t);
  }, [evaluate]);

  const matchedRule = useMemo<Rule | null>(() => {
    if (!result || !config) return null;
    if (result.decision.action !== "open") return null;
    const id = result.decision.matched_rule;
    if (!id) return null;
    return config.rules.find((r) => r.id === id) ?? null;
  }, [result, config]);
  const matchedPosition = useMemo<number | null>(() => {
    if (!matchedRule || !config) return null;
    return config.rules.findIndex((r) => r.id === matchedRule.id) + 1;
  }, [matchedRule, config]);

  return (
    <div className="space-y-4">
      <header>
        <h2 className="mac-h2">Test URL</h2>
        <p className="mac-subtitle">
          Run a URL through the live router without opening a browser. Edit a
          rule, switch back here, and the decision updates instantly.
        </p>
      </header>

      <Card>
        <CardContent className="space-y-3 pt-4">
          <div className="space-y-1.5">
            <Label htmlFor="test-url">URL</Label>
            <Input
              id="test-url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/some/path"
            />
          </div>

          <div className="grid grid-cols-3 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="from-app">From app (optional)</Label>
              <div className="flex gap-2">
                <Input
                  id="from-app"
                  value={fromApp}
                  onChange={(e) => {
                    setFromApp(e.target.value);
                    // Manual edit drops the bundle id (we don't know which
                    // app the typed name refers to). Backend then matches
                    // by name only — which is exactly the desired behavior
                    // for hand-written rules.
                    setFromAppBundleId(null);
                  }}
                  placeholder="Slack, Terminal, VSCode…"
                />
                <AppPickerButton
                  onPicked={(p) => {
                    setFromApp(p.name);
                    setFromAppBundleId(p.bundleId || null);
                  }}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label>From browser (optional)</Label>
              <Select
                value={fromBrowser || NONE}
                onValueChange={(v) => setFromBrowser(v === NONE ? "" : v)}
              >
                <SelectTrigger>
                  <SelectValue placeholder="— none —" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NONE}>— none —</SelectItem>
                  {browsers.map((b) => (
                    <SelectItem key={b.id} value={b.id}>
                      <span className="flex items-center gap-2">
                        <AppIcon
                          bundleId={b.platform_app_id ?? undefined}
                          appPath={appPathFromExecutable(b.executable)}
                          size={16}
                          alt={b.display_name}
                        />
                        {b.display_name}
                      </span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>From profile (optional)</Label>
              <Select
                value={fromProfile || NONE}
                onValueChange={(v) => setFromProfile(v === NONE ? "" : v)}
                disabled={!fromBrowser}
              >
                <SelectTrigger>
                  <SelectValue placeholder="— any —" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NONE}>— any —</SelectItem>
                  {profiles.map((p) => (
                    <SelectItem key={p.id} value={p.id}>
                      {p.display_name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        </CardContent>
      </Card>

      {error && (
        <Card>
          <CardContent className="flex items-center gap-2 pt-4">
            <Badge variant="destructive">error</Badge>
            <span className="text-sm text-muted-foreground">{error}</span>
          </CardContent>
        </Card>
      )}

      {result && (
        <Card>
          <CardHeader>
            <CardTitle>Result</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <ResultRow label="Decision">
              <DecisionLine decision={result.decision} />
            </ResultRow>
            <ResultRow label="Rule">
              {matchedRule ? (
                <>
                  <span
                    className="font-mono text-xs"
                    title="Priority position — top of list wins"
                  >
                    #{matchedPosition}
                  </span>{" "}
                  {matchedRule.note ? (
                    <span className="text-sm">{matchedRule.note}</span>
                  ) : (
                    <span className="text-sm text-muted-foreground">
                      (no note)
                    </span>
                  )}
                </>
              ) : (
                <span className="text-sm text-muted-foreground">
                  — default target (no rule matched)
                </span>
              )}
            </ResultRow>
            <div className="space-y-2">
              <Label>Why this decision</Label>
              <ExplanationView
                explanation={result.explanation}
                emptyMessage="No rule fired. The route would fall back to the configured default_target."
              />
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function ResultRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-start gap-3">
      <span className="w-20 shrink-0 pt-0.5 text-xs text-muted-foreground">
        {label}
      </span>
      <span className="flex-1 min-w-0">{children}</span>
    </div>
  );
}
