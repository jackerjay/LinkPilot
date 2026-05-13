import { useCallback, useEffect, useState } from "react";
import { AppIcon } from "@/components/AppIcon";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { ExplanationView } from "@/components/Explanation";
import { ipc, onRouteLogged } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { DecisionLine } from "./menu-bar";
import type { ConfigDocument, RouteRecord } from "@/lib/types";

export function InspectorPage() {
  const [records, setRecords] = useState<RouteRecord[]>([]);
  const [selected, setSelected] = useState<RouteRecord | null>(null);
  const [config, setConfig] = useState<ConfigDocument | null>(null);
  const [showRaw, setShowRaw] = useState(false);

  const refresh = useCallback(async () => {
    const [recs, doc] = await Promise.all([
      ipc.routeHistory(200),
      ipc.configGet(),
    ]);
    setRecords(recs);
    setConfig(doc);
  }, []);

  useEffect(() => {
    refresh().catch(console.error);
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((record) => {
      setRecords((prev) => [record, ...prev].slice(0, 200));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const matchedRule =
    selected && selected.matched_rule
      ? config?.rules.find((r) => r.id === selected.matched_rule) ?? null
      : null;

  return (
    <div className="space-y-4">
      <header>
        <h2 className="text-xl font-semibold tracking-tight">Route Inspector</h2>
        <p className="text-sm text-muted-foreground">
          Every decision LinkPilot makes, newest first. Click a row to see why
          the rule matched.
        </p>
      </header>

      <Card className="max-h-[480px] overflow-y-auto">
        <CardContent className="p-0">
          {records.length === 0 ? (
            <div className="p-8 text-center text-sm text-muted-foreground">
              No routes logged yet. Click some links or run{" "}
              <span className="font-mono">lp open …</span> while the daemon is
              up.
            </div>
          ) : (
            <div className="divide-y divide-border">
              {records.map((r, i) => (
                <button
                  key={i}
                  type="button"
                  onClick={() => setSelected(r)}
                  className={cn(
                    "flex w-full items-center gap-3 px-4 py-2 text-left transition-colors hover:bg-accent",
                    selected === r && "bg-accent",
                  )}
                >
                  <span className="w-20 shrink-0 font-mono text-xs text-muted-foreground">
                    {new Date(r.timestamp_ms).toLocaleTimeString()}
                  </span>
                  <span className="flex-1 truncate font-mono text-xs">
                    {r.context.url}
                  </span>
                  <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
                    {r.context.source.app_name && (
                      <AppIcon
                        bundleId={r.context.source.bundle_id ?? undefined}
                        size={14}
                        alt={r.context.source.app_name}
                      />
                    )}
                    {r.context.source.app_name ?? r.context.source.type}
                  </span>
                  <DecisionLine decision={r.decision} />
                </button>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {selected && (
        <Card>
          <CardHeader>
            <CardTitle>Selected route</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <SummaryRow label="URL">
              <span className="font-mono text-xs">{selected.context.url}</span>
            </SummaryRow>
            <SummaryRow label="Source">
              {selected.context.source.app_name ? (
                <span className="flex items-center gap-2">
                  <AppIcon
                    bundleId={selected.context.source.bundle_id ?? undefined}
                    size={18}
                    alt={selected.context.source.app_name}
                  />
                  <span className="font-mono text-xs">
                    {selected.context.source.app_name}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    ({selected.context.source.type})
                  </span>
                </span>
              ) : (
                <span className="text-xs text-muted-foreground">
                  {selected.context.source.type}
                </span>
              )}
            </SummaryRow>
            <SummaryRow label="Decision">
              <DecisionLine decision={selected.decision} />
            </SummaryRow>
            <SummaryRow label="Rule">
              {matchedRule ? (
                <>
                  <span className="font-mono text-xs">
                    #{matchedRule.priority}
                  </span>{" "}
                  {matchedRule.note ? (
                    <span className="text-sm">{matchedRule.note}</span>
                  ) : (
                    <span className="text-xs text-muted-foreground">
                      (no note)
                    </span>
                  )}
                </>
              ) : (
                <span className="text-xs text-muted-foreground">
                  — default target (no rule matched)
                </span>
              )}
            </SummaryRow>

            <div className="space-y-2">
              <Label>Why this decision</Label>
              <ExplanationView
                explanation={selected.explanation}
                emptyMessage="No rule fired. The route fell back to the configured default_target."
              />
            </div>

            <div className="flex items-center justify-between border-t border-border pt-3">
              <span className="text-xs text-muted-foreground">
                Raw record (for debugging / bug reports)
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowRaw((v) => !v)}
              >
                {showRaw ? "Hide" : "Show"}
              </Button>
            </div>
            {showRaw && (
              <pre className="whitespace-pre-wrap rounded-md border border-border bg-muted/30 p-3 font-mono text-xs">
                {JSON.stringify(selected, null, 2)}
              </pre>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function SummaryRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-20 shrink-0 text-xs text-muted-foreground">
        {label}
      </span>
      <span className="flex-1">{children}</span>
    </div>
  );
}

