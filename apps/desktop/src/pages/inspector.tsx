import { useCallback, useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
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
  const { t } = useTranslation("inspector");
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
  const matchedPosition =
    matchedRule && config
      ? config.rules.findIndex((r) => r.id === matchedRule.id) + 1
      : null;

  return (
    <div className="space-y-4">
      <header>
        <h2 className="mac-h2">{t("title")}</h2>
        <p className="mac-subtitle">{t("subtitle")}</p>
      </header>

      <Card className="max-h-[480px] overflow-y-auto">
        <CardContent className="p-0">
          {records.length === 0 ? (
            <div className="p-8 text-center text-sm text-muted-foreground">
              <Trans
                i18nKey="emptyHint"
                ns="inspector"
                components={{ code: <span className="font-mono" /> }}
              />
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
                  <span className="min-w-0 flex-1 truncate select-text font-mono text-xs">
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
            <CardTitle>{t("selected.card")}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <SummaryRow label={t("selected.url")}>
              <span className="block select-text break-all font-mono text-xs">
                {selected.context.url}
              </span>
            </SummaryRow>
            <SummaryRow label={t("selected.source")}>
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
            <SummaryRow label={t("selected.decision")}>
              <DecisionLine decision={selected.decision} />
            </SummaryRow>
            <SummaryRow label={t("selected.rule")}>
              {matchedRule ? (
                <>
                  <span
                    className="font-mono text-xs"
                    title={t("selected.rulePriorityTitle")}
                  >
                    #{matchedPosition}
                  </span>{" "}
                  {matchedRule.note ? (
                    <span className="text-sm">{matchedRule.note}</span>
                  ) : (
                    <span className="text-xs text-muted-foreground">
                      {t("selected.ruleNoNote")}
                    </span>
                  )}
                </>
              ) : (
                <span className="text-xs text-muted-foreground">
                  {t("selected.ruleDefault")}
                </span>
              )}
            </SummaryRow>

            <div className="space-y-2">
              <Label>{t("selected.whyTitle")}</Label>
              <ExplanationView
                explanation={selected.explanation}
                emptyMessage={t("selected.whyEmpty")}
              />
            </div>

            <div className="flex items-center justify-between border-t border-border pt-3">
              <span className="text-xs text-muted-foreground">
                {t("selected.rawCaption")}
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowRaw((v) => !v)}
              >
                {showRaw ? t("selected.hide") : t("selected.show")}
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
    <div className="flex items-start gap-3">
      <span className="w-20 shrink-0 pt-0.5 text-xs text-muted-foreground">
        {label}
      </span>
      {/* min-w-0 lets flex-1 actually shrink — without it the child's
          min-width defaults to its content width, so long URLs push the
          row out past the card. Combined with `break-all` on the URL
          span, multi-line URLs now wrap cleanly inside the card. */}
      <span className="flex-1 min-w-0">{children}</span>
    </div>
  );
}

