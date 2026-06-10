// Overview "Activity" card: 24-hour route sparkline + per-browser
// distribution. Pure presentational — computes everything from the
// route history the page already fetches; no extra IPC.

import { useTranslation } from "react-i18next";
import { Activity } from "lucide-react";
import { BrowserBadge } from "@/components/BrowserBadge";
import { EmptyState } from "@/components/EmptyState";
import type { RouteRecord } from "@/lib/types";

const HOURS = 24;
const HOUR_MS = 3_600_000;

// Categorical palette, brand accent first then Apple system colors —
// stays legible on both the light and dark card fills.
const SEGMENT_COLORS = [
  "#5057e8",
  "#30c850",
  "#ff9f0a",
  "#64d2ff",
  "#bf5af2",
  "#ff6482",
];

/** Max legend entries before the tail collapses into "other". */
const LEGEND_MAX = 5;

interface ActivityCardProps {
  history: RouteRecord[];
}

interface BrowserShare {
  /** Browser id, or null for the aggregated "other" tail. */
  browserId: string | null;
  count: number;
}

function hourlyBuckets(history: RouteRecord[], now: number): number[] {
  const buckets: number[] = new Array(HOURS).fill(0);
  for (const r of history) {
    const age = now - r.timestamp_ms;
    if (age < 0 || age >= HOURS * HOUR_MS) continue;
    buckets[HOURS - 1 - Math.floor(age / HOUR_MS)] += 1;
  }
  return buckets;
}

function browserShares(history: RouteRecord[]): BrowserShare[] {
  const counts = new Map<string, number>();
  for (const r of history) {
    if (r.decision.action !== "open") continue;
    const id = r.decision.target.browser;
    counts.set(id, (counts.get(id) ?? 0) + 1);
  }
  const sorted = [...counts.entries()]
    .map(([browserId, count]) => ({ browserId, count }))
    .sort((a, b) => b.count - a.count);
  if (sorted.length <= LEGEND_MAX + 1) return sorted;
  const head: BrowserShare[] = sorted.slice(0, LEGEND_MAX);
  const tail = sorted
    .slice(LEGEND_MAX)
    .reduce((sum, s) => sum + s.count, 0);
  return [...head, { browserId: null, count: tail }];
}

function hourLabel(now: number, bucketIndex: number): string {
  const d = new Date(now - (HOURS - 1 - bucketIndex) * HOUR_MS);
  return `${String(d.getHours()).padStart(2, "0")}:00`;
}

export function ActivityCard({ history }: ActivityCardProps) {
  const { t } = useTranslation("menuBar");
  const now = Date.now();
  const buckets = hourlyBuckets(history, now);
  const maxBucket = Math.max(...buckets);
  const shares = browserShares(history);
  const totalOpens = shares.reduce((sum, s) => sum + s.count, 0);

  return (
    <>
      <div className="mac-card-title">{t("activity.card")}</div>
      <div className="mac-card" style={{ padding: "14px 16px" }}>
        {history.length === 0 ? (
          <EmptyState
            icon={Activity}
            title={t("activity.emptyTitle")}
            hint={t("activity.emptyHint")}
          />
        ) : (
          <>
            <div
              className="mac-row-label"
              style={{ fontSize: 12, marginBottom: 10 }}
            >
              {t("activity.last24h")}
            </div>
            <div
              style={{
                display: "flex",
                alignItems: "flex-end",
                gap: 3,
                height: 44,
              }}
            >
              {buckets.map((count, i) => (
                <div
                  key={i}
                  title={`${hourLabel(now, i)} · ${count}`}
                  style={{
                    flex: 1,
                    height:
                      count > 0
                        ? Math.max(6, Math.round((count / maxBucket) * 44))
                        : 3,
                    borderRadius: 2,
                    background:
                      count > 0 ? "var(--mac-accent)" : "var(--mac-inset-fill)",
                    opacity:
                      count > 0 ? 0.45 + 0.55 * (count / maxBucket) : 1,
                  }}
                />
              ))}
            </div>
            <div
              className="mac-muted"
              style={{
                display: "flex",
                justifyContent: "space-between",
                fontSize: 10.5,
                marginTop: 4,
                fontVariantNumeric: "tabular-nums",
              }}
            >
              <span>−24h</span>
              <span>{t("activity.now")}</span>
            </div>

            {shares.length > 0 && (
              <>
                <div
                  style={{
                    height: 0.5,
                    background: "var(--mac-divider)",
                    margin: "14px 0",
                  }}
                />
                <div
                  className="mac-row-label"
                  style={{ fontSize: 12, marginBottom: 10 }}
                >
                  {t("activity.byBrowser")}
                </div>
                <div
                  style={{
                    display: "flex",
                    height: 8,
                    borderRadius: 4,
                    overflow: "hidden",
                    gap: 1,
                    marginBottom: 10,
                  }}
                >
                  {shares.map((s, i) => (
                    <div
                      key={s.browserId ?? "__other"}
                      style={{
                        width: `${(s.count / totalOpens) * 100}%`,
                        minWidth: 3,
                        background:
                          SEGMENT_COLORS[i % SEGMENT_COLORS.length],
                      }}
                    />
                  ))}
                </div>
                <div style={{ display: "grid", gap: 4 }}>
                  {shares.map((s, i) => (
                    <div
                      key={s.browserId ?? "__other"}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 8,
                        fontSize: 12,
                      }}
                    >
                      <span
                        style={{
                          width: 8,
                          height: 8,
                          borderRadius: "50%",
                          flexShrink: 0,
                          background:
                            SEGMENT_COLORS[i % SEGMENT_COLORS.length],
                        }}
                      />
                      <span
                        className="grow"
                        style={{ minWidth: 0, overflow: "hidden" }}
                      >
                        {s.browserId !== null ? (
                          <BrowserBadge browserId={s.browserId} />
                        ) : (
                          <span className="mac-muted">
                            {t("activity.other")}
                          </span>
                        )}
                      </span>
                      <span
                        className="mac-row-value"
                        style={{ fontSize: 12 }}
                      >
                        {s.count}
                        <span className="mac-muted">
                          {" "}
                          · {Math.round((s.count / totalOpens) * 100)}%
                        </span>
                      </span>
                    </div>
                  ))}
                </div>
              </>
            )}
          </>
        )}
      </div>
    </>
  );
}
