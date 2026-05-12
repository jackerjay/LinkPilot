// Cascading browser + profile dropdown for a BrowserTarget. Used by:
// - RuleEditor (the "open → X" action of a rule)
// - Settings (the default_target shown when no rule matches)

import { useEffect, useState } from "react";
import { ipc } from "../lib/ipc";
import type {
  BrowserProfile,
  BrowserTarget,
  InstalledBrowser,
} from "../lib/types";

interface Props {
  value: BrowserTarget;
  browsers: InstalledBrowser[];
  onChange: (next: BrowserTarget) => void;
}

export function TargetEditor({ value, browsers, onChange }: Props) {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [profilesError, setProfilesError] = useState<string | null>(null);

  useEffect(() => {
    if (!value.browser) {
      setProfiles([]);
      setProfilesError(null);
      return;
    }
    let alive = true;
    ipc
      .listProfiles(value.browser)
      .then((p) => {
        if (alive) {
          setProfiles(p);
          setProfilesError(null);
        }
      })
      .catch((e) => {
        if (alive) {
          setProfiles([]);
          setProfilesError(String(e));
        }
      });
    return () => {
      alive = false;
    };
  }, [value.browser]);

  return (
    <>
      <select
        value={value.browser}
        onChange={(e) =>
          onChange({ ...value, browser: e.target.value, profile: null })
        }
      >
        <option value="">— pick a browser —</option>
        {browsers.map((b) => (
          <option key={b.id} value={b.id}>
            {b.display_name}
          </option>
        ))}
      </select>
      <select
        value={value.profile ?? ""}
        onChange={(e) =>
          onChange({ ...value, profile: e.target.value || null })
        }
        disabled={!value.browser}
        title={profilesError ?? ""}
      >
        <option value="">(any profile)</option>
        {profiles.map((p) => (
          <option key={p.id} value={p.id}>
            {p.display_name}
          </option>
        ))}
      </select>
      <label
        className="muted"
        style={{ display: "flex", gap: 4, alignItems: "center" }}
      >
        <input
          type="checkbox"
          checked={value.incognito ?? false}
          onChange={(e) =>
            onChange({ ...value, incognito: e.target.checked || undefined })
          }
        />
        incognito
      </label>
    </>
  );
}
