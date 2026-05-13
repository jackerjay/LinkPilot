// Cascading browser + profile picker for a BrowserTarget. Used by:
// - RuleEditor (the "open → X" action of a rule)
// - Settings (the default_target shown when no rule matches)

import { useEffect, useState } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ipc } from "@/lib/ipc";
import type {
  BrowserProfile,
  BrowserTarget,
  InstalledBrowser,
} from "@/lib/types";

interface Props {
  value: BrowserTarget;
  browsers: InstalledBrowser[];
  onChange: (next: BrowserTarget) => void;
}

export function TargetEditor({ value, browsers, onChange }: Props) {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);

  useEffect(() => {
    if (!value.browser) {
      setProfiles([]);
      return;
    }
    let alive = true;
    ipc
      .listProfiles(value.browser)
      .then((p) => {
        if (alive) setProfiles(p);
      })
      .catch(() => {
        if (alive) setProfiles([]);
      });
    return () => {
      alive = false;
    };
  }, [value.browser]);

  return (
    <>
      <Select
        value={value.browser || undefined}
        onValueChange={(v) =>
          onChange({ ...value, browser: v, profile: null })
        }
      >
        <SelectTrigger className="w-[180px]">
          <SelectValue placeholder="— pick a browser —" />
        </SelectTrigger>
        <SelectContent>
          {browsers.map((b) => (
            <SelectItem key={b.id} value={b.id}>
              {b.display_name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Select
        value={value.profile ?? "__any"}
        onValueChange={(v) =>
          onChange({ ...value, profile: v === "__any" ? null : v })
        }
        disabled={!value.browser}
      >
        <SelectTrigger className="w-[160px]">
          <SelectValue placeholder="(any profile)" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="__any">(any profile)</SelectItem>
          {profiles.map((p) => (
            <SelectItem key={p.id} value={p.id}>
              {p.display_name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Checkbox
          checked={value.incognito ?? false}
          onCheckedChange={(v) =>
            onChange({ ...value, incognito: v === true ? true : undefined })
          }
        />
        incognito
      </label>
    </>
  );
}
