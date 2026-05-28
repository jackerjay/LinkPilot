// SuggestionsPanel — Rules-page surface for the ask-mode behavior log.
//
// Reads aggregated `Suggestion`s from `suggestions_list` (Rust:
// observations.rs + suggestions.rs), renders one card row per pattern,
// and exposes two one-click actions:
//   • "Make rule" → builds a `Rule { when: url-host, then: open }` from
//     the suggestion and calls the same `ipc.ruleUpsert` path the Rules
//     editor uses. The parent refreshes its rule list via `onRuleCreated`.
//   • "Dismiss" → marks the (host, browser_id, profile_id) tuple muted
//     for 30 days. The row disappears immediately.
//
// Empty state: the panel renders nothing. Rules-page real estate is
// precious; the suggestion is opt-in noise the user only wants when
// it's actionable.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, X } from "lucide-react";

import { BrowserBadge } from "@/components/BrowserBadge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ipc } from "@/lib/ipc";
import { browserDisplayName, useBrowsers } from "@/lib/browsers";
import type { Rule, Suggestion } from "@/lib/types";

interface Props {
  /** Refresh trigger from the parent. Bumping this re-fetches the
   *  suggestion list — used after the parent saves an unrelated rule
   *  so any newly-shadowed suggestion drops out. */
  refreshKey?: number;
  /** Called after the user accepts a suggestion → a fresh rule lands in
   *  the config. The Rules page passes its own `refresh` so the list
   *  updates without waiting for the next fsnotify echo. */
  onRuleCreated?: () => void;
}

export function SuggestionsPanel({ refreshKey, onRuleCreated }: Props) {
  const { t } = useTranslation("suggestions");
  const browsers = useBrowsers();
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await ipc.suggestionsList();
      setSuggestions(next);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshKey]);

  const acceptSuggestion = useCallback(
    async (suggestion: Suggestion) => {
      const key = rowKey(suggestion);
      setBusyKey(key);
      try {
        const rule: Rule = {
          id: crypto.randomUUID(),
          enabled: true,
          when: { op: "url-host", pattern: suggestion.host },
          then: {
            kind: "open",
            target: {
              browser: suggestion.browser_id,
              profile: suggestion.profile_id ?? null,
            },
          },
          source: "gui",
          note: "Suggested from your ask-picker history",
        };
        await ipc.ruleUpsert(rule);
        // Mute the exact suggestion so it can't re-appear before the
        // fsnotify-driven refresh kicks in. (Even if we made a rule,
        // the user might disable it; suppressing keeps things tidy.)
        await ipc
          .suggestionsDismiss(
            suggestion.host,
            suggestion.browser_id,
            suggestion.profile_id ?? null,
          )
          .catch(() => undefined);
        await refresh();
        onRuleCreated?.();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusyKey(null);
      }
    },
    [onRuleCreated, refresh],
  );

  const dismissSuggestion = useCallback(
    async (suggestion: Suggestion) => {
      const key = rowKey(suggestion);
      setBusyKey(key);
      try {
        await ipc.suggestionsDismiss(
          suggestion.host,
          suggestion.browser_id,
          suggestion.profile_id ?? null,
        );
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusyKey(null);
      }
    },
    [refresh],
  );

  if (suggestions.length === 0 && !error) {
    return null;
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("panel.title")}</CardTitle>
        <p className="mac-subtitle">{t("panel.description")}</p>
      </CardHeader>
      <CardContent>
        {error && (
          <p className="mb-2 text-sm text-red-500" role="alert">
            {error}
          </p>
        )}
        <ul className="space-y-2">
          {suggestions.map((s) => {
            const key = rowKey(s);
            const busy = busyKey === key;
            const browserName =
              browserDisplayName(s.browser_id, browsers) || s.browser_id;
            const percent = Math.round(s.confidence * 100);
            return (
              <li
                key={key}
                aria-label={t("panel.rowAria", {
                  host: s.host,
                  browser: browserName,
                })}
                className="flex items-center justify-between gap-3 rounded-md border border-border/60 bg-card/60 p-3"
              >
                <div className="flex min-w-0 flex-1 items-center gap-3">
                  <BrowserBadge
                    browserId={s.browser_id}
                    profile={s.profile_id}
                    iconSize={16}
                  />
                  <span className="text-muted-foreground">→</span>
                  <span
                    className="truncate font-mono text-sm"
                    title={s.host}
                  >
                    {s.host}
                  </span>
                  <Badge variant="secondary" className="ml-1 shrink-0">
                    {t("panel.confidence", { percent })}
                  </Badge>
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {t("panel.countLabel", { count: s.observation_count })}
                  </span>
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  <Button
                    size="sm"
                    variant="default"
                    disabled={busy}
                    onClick={() => void acceptSuggestion(s)}
                    title={t("panel.makeRuleTooltip", {
                      host: s.host,
                      browser: browserName,
                    })}
                  >
                    <Check className="mr-1 h-3.5 w-3.5" />
                    {t("panel.makeRule")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => void dismissSuggestion(s)}
                    title={t("panel.dismissTooltip")}
                    aria-label={t("panel.dismiss")}
                  >
                    <X className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </li>
            );
          })}
        </ul>
      </CardContent>
    </Card>
  );
}

function rowKey(s: Suggestion): string {
  return `${s.host}|${s.browser_id}|${s.profile_id ?? ""}`;
}
