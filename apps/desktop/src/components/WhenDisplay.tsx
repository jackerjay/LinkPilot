// Render a MatcherTree as inline JSX with a context-appropriate icon
// per leaf:
//   url-host       → Globe          (web hostname)
//   url-path       → Link2          (URL path slice)
//   source-app     → AppIcon (real macOS icon resolved by Spotlight)
//   source-browser → BrowserBadge   (browser icon from inventory)
//   source-profile → User
//   always         → Asterisk
//   AND / OR / NOT compose children inline with a subtle separator badge.
//
// Replaces the previous text-only `describeWhen()` function in
// pages/rules.tsx — same content shape, just visual.

import { Fragment } from "react";
import { useTranslation } from "react-i18next";
import {
  Asterisk,
  Ban,
  Globe,
  Layers,
  Link2,
  Split,
  User,
} from "lucide-react";
import { AppIcon } from "@/components/AppIcon";
import { BrowserBadge } from "@/components/BrowserBadge";
import type { MatcherTree } from "@/lib/types";

interface Props {
  matcher: MatcherTree;
  /** Icon size in px. */
  iconSize?: number;
}

export function WhenDisplay({ matcher, iconSize = 14 }: Props) {
  const { t } = useTranslation("rules");
  switch (matcher.op) {
    case "always":
      return (
        <span className="inline-flex items-center gap-1.5">
          <Asterisk
            className="shrink-0 text-amber-500"
            style={{ width: iconSize, height: iconSize }}
          />
          {t("matcherLabels.always")}
        </span>
      );

    case "url-host":
      return (
        <span className="inline-flex items-center gap-1.5">
          <Globe
            className="shrink-0 text-sky-500"
            style={{ width: iconSize, height: iconSize }}
          />
          <span className="font-mono">{matcher.pattern}</span>
        </span>
      );

    case "url-path":
      return (
        <span className="inline-flex items-center gap-1.5">
          <Link2
            className="shrink-0 text-emerald-500"
            style={{ width: iconSize, height: iconSize }}
          />
          <span className="text-muted-foreground">
            {t("matcherLabels.path")}
          </span>
          <span className="font-mono">{matcher.pattern}</span>
        </span>
      );

    case "source-app":
      return (
        <span className="inline-flex items-center gap-1.5">
          <AppIcon
            // Picker-authored rules store bundle_id — most reliable
            // lookup. Hand-typed / older rules fall back to name.
            bundleId={matcher.bundle_id ?? undefined}
            name={matcher.bundle_id ? undefined : matcher.name}
            size={iconSize}
            alt={matcher.name}
          />
          <span className="text-muted-foreground">
            {t("matcherLabels.from")}
          </span>
          <span>{matcher.name}</span>
        </span>
      );

    case "source-browser":
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="text-muted-foreground">
            {t("matcherLabels.from")}
          </span>
          <BrowserBadge browserId={matcher.browser} iconSize={iconSize} />
        </span>
      );

    case "source-profile":
      return (
        <span className="inline-flex items-center gap-1.5">
          <User
            className="shrink-0 text-violet-500"
            style={{ width: iconSize, height: iconSize }}
          />
          <span className="text-muted-foreground">
            {t("matcherLabels.profile")}
          </span>
          <span className="font-mono">{matcher.profile}</span>
        </span>
      );

    case "all":
      return (
        <ComposedDisplay
          icon={
            <Layers
              className="shrink-0 text-indigo-500"
              style={{ width: iconSize, height: iconSize }}
            />
          }
          separator={t("matcherLabels.and")}
          children_={matcher.of}
          iconSize={iconSize}
        />
      );

    case "any":
      return (
        <ComposedDisplay
          icon={
            <Split
              className="shrink-0 text-cyan-500"
              style={{ width: iconSize, height: iconSize }}
            />
          }
          separator={t("matcherLabels.or")}
          children_={matcher.of}
          iconSize={iconSize}
        />
      );

    case "not":
      return (
        <span className="inline-flex items-center gap-1.5">
          <Ban
            className="shrink-0 text-rose-500"
            style={{ width: iconSize, height: iconSize }}
          />
          <span className="text-rose-500">{t("matcherLabels.not")}</span>
          <WhenDisplay matcher={matcher.of} iconSize={iconSize} />
        </span>
      );
  }
}

function ComposedDisplay({
  icon,
  separator,
  children_,
  iconSize,
}: {
  icon: React.ReactNode;
  separator: string;
  children_: MatcherTree[];
  iconSize: number;
}) {
  return (
    <span className="inline-flex items-center gap-1.5">
      {icon}
      {children_.map((child, idx) => (
        <Fragment key={idx}>
          {idx > 0 && (
            <span className="text-xs uppercase tracking-wider text-muted-foreground/60">
              {separator}
            </span>
          )}
          <WhenDisplay matcher={child} iconSize={iconSize} />
        </Fragment>
      ))}
    </span>
  );
}
