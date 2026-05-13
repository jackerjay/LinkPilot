// One-stop label for a browser target: icon + properly capitalized
// display name + optional profile suffix. Used everywhere we previously
// rendered raw lowercase browser ids (Rules action column, Default
// target header, Inspector DecisionLine, Test URL result, ...).

import { AppIcon } from "@/components/AppIcon";
import {
  appPathFromExecutable,
  browserDisplayName,
  useBrowsers,
} from "@/lib/browsers";
import { cn } from "@/lib/utils";

interface Props {
  browserId: string;
  profile?: string | null;
  /** Pixel size of the icon. Defaults to 14 (matches body text height). */
  iconSize?: number;
  /** Extra utility classes on the outer span. */
  className?: string;
  /** Render the profile in muted color when present. */
  mutedProfile?: boolean;
}

export function BrowserBadge({
  browserId,
  profile,
  iconSize = 14,
  className,
  mutedProfile = true,
}: Props) {
  const browsers = useBrowsers();
  const installed = browsers.find((b) => b.id === browserId);
  const name = browserDisplayName(browserId, browsers);

  return (
    <span className={cn("inline-flex items-center gap-1.5", className)}>
      <AppIcon
        bundleId={installed?.platform_app_id ?? undefined}
        appPath={
          installed ? appPathFromExecutable(installed.executable) : undefined
        }
        size={iconSize}
        alt={name}
      />
      <span>{name}</span>
      {profile && (
        <span className={mutedProfile ? "text-muted-foreground" : undefined}>
          / {profile}
        </span>
      )}
    </span>
  );
}
