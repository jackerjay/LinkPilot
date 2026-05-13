import { useCallback, useEffect, useState } from "react";
import { BrowserBadge } from "@/components/BrowserBadge";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ipc, onRouteLogged } from "@/lib/ipc";
import type { DoctorReport, RouteRecord, RoutingDecision } from "@/lib/types";

interface Props {
  configEpoch: number;
}

export function MenuBarPage({ configEpoch }: Props) {
  const [doctor, setDoctor] = useState<DoctorReport | null>(null);
  const [recent, setRecent] = useState<RouteRecord[]>([]);

  const refresh = useCallback(async () => {
    setDoctor(await ipc.doctor());
    setRecent(await ipc.routeHistory(5));
  }, []);

  useEffect(() => {
    refresh().catch(console.error);
  }, [refresh, configEpoch]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onRouteLogged((record) => {
      setRecent((prev) => [record, ...prev].slice(0, 5));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <div className="space-y-4">
      <header>
        <h2 className="text-xl font-semibold tracking-tight">Overview</h2>
        <p className="text-sm text-muted-foreground">
          Daemon status and the most recent routing decisions. Use the{" "}
          <em>Test URL</em> tab to dry-run a URL through the router.
        </p>
      </header>

      <Card>
        <CardHeader>
          <CardTitle>Status</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          <StatusRow label="Daemon version" value={doctor?.daemon_version ?? "…"} mono />
          <div className="flex items-center justify-between">
            <span className="text-sm">LinkPilot is default browser</span>
            <Badge variant={doctor?.is_default_browser ? "success" : "destructive"}>
              {doctor?.is_default_browser ? "yes" : "no"}
            </Badge>
          </div>
          <StatusRow
            label="Installed browsers detected"
            value={String(doctor?.installed_browser_count ?? 0)}
          />
          <StatusRow
            label="Config file"
            value={doctor?.config_path ?? "…"}
            mono
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Recent routes</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {recent.length === 0 ? (
            <div className="p-8 text-center text-sm text-muted-foreground">
              No routes yet. Try <span className="font-mono">lp open …</span> or
              click a link.
            </div>
          ) : (
            <div className="divide-y divide-border">
              {recent.map((r, i) => (
                <RouteRow key={i} record={r} />
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function StatusRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-sm">{label}</span>
      <span
        className={
          mono
            ? "select-text font-mono text-xs text-muted-foreground"
            : "select-text text-sm text-muted-foreground"
        }
      >
        {value}
      </span>
    </div>
  );
}

function RouteRow({ record }: { record: RouteRecord }) {
  return (
    <div className="flex items-center gap-3 px-4 py-2">
      <span className="w-20 shrink-0 font-mono text-xs text-muted-foreground">
        {new Date(record.timestamp_ms).toLocaleTimeString()}
      </span>
      <span className="flex-1 truncate select-text font-mono text-xs">
        {record.context.url}
      </span>
      <DecisionLine decision={record.decision} />
    </div>
  );
}

export function DecisionLine({ decision }: { decision: RoutingDecision }) {
  if (decision.action === "open") {
    return (
      <div className="flex items-center gap-2">
        <Badge variant="default">open</Badge>
        <BrowserBadge
          browserId={decision.target.browser}
          profile={decision.target.profile}
          className="text-xs"
        />
      </div>
    );
  }
  if (decision.action === "allow") {
    return (
      <div className="flex items-center gap-2">
        <Badge variant="accent">allow</Badge>
        <span className="text-xs text-muted-foreground">{decision.reason}</span>
      </div>
    );
  }
  if (decision.action === "block") {
    return (
      <div className="flex items-center gap-2">
        <Badge variant="destructive">block</Badge>
        <span className="text-xs text-muted-foreground">{decision.reason}</span>
      </div>
    );
  }
  return (
    <div className="flex items-center gap-2">
      <Badge variant="secondary">ask</Badge>
      <span className="text-xs text-muted-foreground">{decision.reason}</span>
    </div>
  );
}
