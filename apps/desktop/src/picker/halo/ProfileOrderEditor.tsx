// Per-browser profile order editor for Settings → Appearance.
// Up/down arrow buttons (no drag-and-drop — would need a new dependency
// and the wheel already constrains us to ≤9 keyboard slots, so most
// reorderings are a few clicks).
//
// Order semantics live on the backend (see core::browser::apply_profile_order):
// unlisted profiles append at the tail in default sort, so this editor only
// ever needs to remember the prefix the user actually customized.

import { useCallback, useEffect, useMemo, useState } from "react";
import { AppIcon } from "@/components/AppIcon";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ipc } from "@/lib/ipc";
import { appPathFromExecutable } from "@/lib/browsers";
import type {
  BrowserProfile,
  ConfigDocument,
  InstalledBrowser,
} from "@/lib/types";
import { FALLBACK_PALETTE, profileMonogram } from "./types";

interface ProfileOrderEditorProps {
  doc: ConfigDocument | null;
  /** Re-fetch the config after mutating. Parent owns the
   *  ConfigDocument; we just signal "go reload". */
  onConfigChanged: () => Promise<void> | void;
}

export function ProfileOrderEditor({
  doc,
  onConfigChanged,
}: ProfileOrderEditorProps) {
  const [browsers, setBrowsers] = useState<InstalledBrowser[]>([]);
  const [selectedBrowser, setSelectedBrowser] = useState<string>("");
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Browsers that have >1 profile are the only ones where ordering
  // matters. Single-profile (Safari) gets nothing to arrange.
  const multiProfileBrowsers = useMemo(
    () =>
      // We don't know profile counts until listProfiles fires; show
      // every browser for now and the editor empties itself out when
      // the selected one has fewer than 2 profiles.
      browsers,
    [browsers],
  );

  useEffect(() => {
    ipc
      .listBrowsers()
      .then((bs) => {
        setBrowsers(bs);
        // Auto-select the first browser so the editor isn't empty on
        // first open. The user can still re-pick from the dropdown.
        if (!selectedBrowser && bs.length > 0) {
          setSelectedBrowser(bs[0].id);
        }
      })
      .catch((e) => setError(String(e)));
    // We deliberately don't depend on selectedBrowser — refetching
    // browsers shouldn't reset the user's pick.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reload profiles when the selected browser changes — applying the
  // saved order if one exists. Profiles not in the saved order get
  // sorted default-first/alpha and pushed to the tail, mirroring the
  // backend's `apply_profile_order` so the editor reflects exactly
  // what the picker would render.
  useEffect(() => {
    if (!selectedBrowser) {
      setProfiles([]);
      return;
    }
    let alive = true;
    ipc
      .listProfiles(selectedBrowser)
      .then((raw) => {
        if (!alive) return;
        const saved = doc?.settings.profile_orders?.[selectedBrowser];
        setProfiles(applyOrderClientSide(raw, saved));
      })
      .catch(() => {
        if (alive) setProfiles([]);
      });
    return () => {
      alive = false;
    };
    // Reapply when the saved order in `doc` changes (e.g. after we
    // call setProfileOrder and the parent refreshes the doc).
  }, [selectedBrowser, doc]);

  const move = useCallback(
    (idx: number, direction: -1 | 1) => {
      setProfiles((prev) => {
        const next = [...prev];
        const target = idx + direction;
        if (target < 0 || target >= next.length) return prev;
        [next[idx], next[target]] = [next[target], next[idx]];
        // Persist immediately — Settings pages in this codebase don't
        // have explicit Save buttons, every mutation writes through.
        ipc
          .setProfileOrder(
            selectedBrowser,
            next.map((p) => p.id),
          )
          .then(() => onConfigChanged())
          .catch((e) => setError(String(e)));
        return next;
      });
    },
    [selectedBrowser, onConfigChanged],
  );

  const reset = useCallback(() => {
    // Empty list clears the saved order — backend falls back to
    // default-first/alpha sort.
    ipc
      .setProfileOrder(selectedBrowser, [])
      .then(() => onConfigChanged())
      .catch((e) => setError(String(e)));
  }, [selectedBrowser, onConfigChanged]);

  const hasSavedOrder = useMemo(() => {
    if (!doc || !selectedBrowser) return false;
    const saved = doc.settings.profile_orders?.[selectedBrowser];
    return saved != null && saved.length > 0;
  }, [doc, selectedBrowser]);

  if (browsers.length === 0) {
    return (
      <div className="profile-order-empty">
        No browsers detected yet. Install a Chromium-family browser to
        customize profile order.
      </div>
    );
  }

  return (
    <div className="profile-order-editor">
      <div className="profile-order-head">
        <Select
          value={selectedBrowser}
          onValueChange={(v) => setSelectedBrowser(v)}
        >
          <SelectTrigger className="w-[180px]">
            <SelectValue placeholder="Pick a browser" />
          </SelectTrigger>
          <SelectContent>
            {multiProfileBrowsers.map((b) => (
              <SelectItem key={b.id} value={b.id}>
                <span className="profile-order-browser-row">
                  <AppIcon
                    bundleId={b.platform_app_id ?? undefined}
                    appPath={appPathFromExecutable(b.executable)}
                    size={14}
                    alt={b.display_name}
                  />
                  {b.display_name}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {hasSavedOrder && (
          <button
            type="button"
            className="mac-tbtn"
            onClick={reset}
            title="Forget the saved order; picker falls back to default"
          >
            Reset
          </button>
        )}
      </div>

      {profiles.length === 0 ? (
        <div className="profile-order-empty">
          No profiles detected for this browser.
        </div>
      ) : profiles.length === 1 ? (
        <div className="profile-order-empty">
          {profiles[0].display_name} is the only profile — nothing to
          reorder.
        </div>
      ) : (
        <ol className="profile-order-list">
          {profiles.map((p, idx) => {
            const accent =
              p.accent_color ?? FALLBACK_PALETTE[idx % FALLBACK_PALETTE.length];
            return (
              <li key={p.id} className="profile-order-row">
                <span
                  className="profile-order-num"
                  title={
                    idx < 9
                      ? `Press ${idx + 1} in the picker to launch this`
                      : "Past slot 9 — keyboard shortcut unavailable"
                  }
                >
                  {idx + 1}
                </span>
                <span
                  className="profile-order-avatar"
                  style={{ background: accent }}
                >
                  {profileMonogram({
                    id: p.id,
                    name: p.display_name,
                    is_default: !!p.is_default,
                  })}
                </span>
                <span className="profile-order-name">
                  <span>{p.display_name}</span>
                  {p.email && (
                    <span className="profile-order-email">{p.email}</span>
                  )}
                  {p.is_default && (
                    <span className="profile-order-default-tag">DEFAULT</span>
                  )}
                </span>
                <span className="profile-order-actions">
                  <button
                    type="button"
                    aria-label="Move up"
                    disabled={idx === 0}
                    onClick={() => move(idx, -1)}
                  >
                    ▲
                  </button>
                  <button
                    type="button"
                    aria-label="Move down"
                    disabled={idx === profiles.length - 1}
                    onClick={() => move(idx, 1)}
                  >
                    ▼
                  </button>
                </span>
              </li>
            );
          })}
        </ol>
      )}

      {error && <div className="profile-order-error">{error}</div>}
    </div>
  );
}

/** Mirror of `core::browser::apply_profile_order` so the editor renders
 *  immediately without an extra round-trip. The backend re-applies the
 *  same logic when the picker actually opens, so any divergence would
 *  show up the moment the user clicked "Try picker". */
function applyOrderClientSide(
  raw: BrowserProfile[],
  saved: string[] | undefined,
): BrowserProfile[] {
  if (!saved || saved.length === 0) {
    return [...raw].sort(
      (a, b) =>
        Number(!!b.is_default) - Number(!!a.is_default) ||
        a.display_name.localeCompare(b.display_name),
    );
  }
  const byId = new Map(raw.map((p) => [p.id, p]));
  const out: BrowserProfile[] = [];
  for (const id of saved) {
    const p = byId.get(id);
    if (p) {
      out.push(p);
      byId.delete(id);
    }
  }
  const leftovers = [...byId.values()].sort(
    (a, b) =>
      Number(!!b.is_default) - Number(!!a.is_default) ||
      a.display_name.localeCompare(b.display_name),
  );
  return [...out, ...leftovers];
}
