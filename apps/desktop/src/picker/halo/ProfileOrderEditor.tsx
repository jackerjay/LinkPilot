// Per-browser profile visibility/order editor for Settings → Appearance.
// The saved list is the complete visible order for that browser. An empty
// list clears customization and falls back to browser-default ordering.

import * as Dialog from "@radix-ui/react-dialog";
import { RotateCcw, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { AppIcon } from "@/components/AppIcon";
import { appPathFromExecutable } from "@/lib/browsers";
import { ipc } from "@/lib/ipc";
import type {
  BrowserProfile,
  ConfigDocument,
  InstalledBrowser,
  PickerStyle,
} from "@/lib/types";
import { polarToCart, sectorMidAngle } from "./geometry";
import { FALLBACK_PALETTE, profileMonogram, type PickerProfile } from "./types";
import { HaloPreview } from "./HaloPreview";

interface ProfileOrderEditorProps {
  doc: ConfigDocument | null;
  pickerStyle: PickerStyle;
  /** Re-fetch the config after mutating. Parent owns the ConfigDocument. */
  onConfigChanged: () => Promise<void> | void;
}

interface BrowserProfileCatalog {
  browser: InstalledBrowser;
  profiles: BrowserProfile[];
  error: string | null;
}

interface InitialDraft {
  profiles: BrowserProfile[];
  clearOnSave: boolean;
}

const EDITOR_HALO_SIZE = 360;

export function ProfileOrderEditor({
  doc,
  pickerStyle,
  onConfigChanged,
}: ProfileOrderEditorProps) {
  const { t } = useTranslation("picker");
  const [catalog, setCatalog] = useState<BrowserProfileCatalog[]>([]);
  const [editingBrowserId, setEditingBrowserId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setError(null);

    ipc
      .listBrowsers()
      .then(async (browsers) => {
        const rows = await Promise.all(
          browsers.map(async (browser): Promise<BrowserProfileCatalog> => {
            try {
              const profiles = await ipc.listProfiles(browser.id);
              return { browser, profiles, error: null };
            } catch (e) {
              return { browser, profiles: [], error: String(e) };
            }
          }),
        );
        if (!alive) return;
        setCatalog(rows);
      })
      .catch((e) => {
        if (alive) {
          setCatalog([]);
          setError(String(e));
        }
      })
      .finally(() => {
        if (alive) setLoading(false);
      });

    return () => {
      alive = false;
    };
  }, []);

  const editingRow = useMemo(
    () => catalog.find((row) => row.browser.id === editingBrowserId) ?? null,
    [catalog, editingBrowserId],
  );

  const configurableCount = catalog.filter(
    (row) => row.profiles.length > 1,
  ).length;

  // Per-browser: profiles the user has detected but never placed into the
  // saved Halo order. This happens organically when someone creates a new
  // Chrome profile after customizing their LinkPilot wheel — without this
  // signal the new profile would stay invisible in the picker until the
  // user remembered to come back here.
  const hiddenByBrowser = useMemo(() => {
    const out = new Map<string, number>();
    for (const row of catalog) {
      const saved = doc?.settings.profile_orders?.[row.browser.id];
      if (!saved || saved.length === 0) continue;
      const savedSet = new Set(saved);
      const hidden = row.profiles.filter((p) => !savedSet.has(p.id)).length;
      if (hidden > 0) out.set(row.browser.id, hidden);
    }
    return out;
  }, [catalog, doc]);
  const hiddenSummary = useMemo(() => {
    let total = 0;
    const browserNames: string[] = [];
    for (const row of catalog) {
      const n = hiddenByBrowser.get(row.browser.id) ?? 0;
      if (n > 0) {
        total += n;
        browserNames.push(row.browser.display_name);
      }
    }
    return { total, browserNames };
  }, [catalog, hiddenByBrowser]);

  if (loading) {
    return <div className="profile-order-empty">{t("profileOrder.scanning")}</div>;
  }

  if (catalog.length === 0) {
    return (
      <div className="profile-order-empty">
        {t("profileOrder.noBrowsers")}
      </div>
    );
  }

  return (
    <Dialog.Root
      open={editingRow !== null}
      onOpenChange={(open) => {
        if (!open) setEditingBrowserId(null);
      }}
    >
      <div className="profile-order-editor">
        {hiddenSummary.total > 0 && (
          <div
            className="profile-order-hidden-banner"
            role="status"
            aria-live="polite"
          >
            <span className="profile-order-hidden-banner-dot" aria-hidden />
            <span className="profile-order-hidden-banner-copy">
              <Trans
                i18nKey="profileOrder.hiddenBanner"
                ns="picker"
                count={hiddenSummary.total}
                values={{
                  count: hiddenSummary.total,
                  browsers: formatBrowserList(t, hiddenSummary.browserNames),
                }}
                components={{ strong: <strong /> }}
              />
            </span>
          </div>
        )}
        <div className="profile-order-browser-grid">
          {catalog.map((row) => {
            const canConfigure = row.profiles.length > 1;
            const saved = hasSavedOrder(doc, row.browser.id);
            const hiddenCount = hiddenByBrowser.get(row.browser.id) ?? 0;
            return (
              <button
                key={row.browser.id}
                type="button"
                className={`profile-order-browser-card${
                  canConfigure ? "" : " disabled"
                }${saved ? " saved" : ""}${
                  hiddenCount > 0 ? " has-hidden" : ""
                }`}
                disabled={!canConfigure}
                onClick={() => setEditingBrowserId(row.browser.id)}
                  title={
                    hiddenCount > 0
                    ? t("profileOrder.hiddenTitle", {
                        count: hiddenCount,
                      })
                    : canConfigure
                      ? t("profileOrder.configureTitle")
                      : t("profileOrder.unavailableTitle")
                }
              >
                <span className="profile-order-browser-icon">
                  <AppIcon
                    bundleId={row.browser.platform_app_id ?? undefined}
                    appPath={appPathFromExecutable(row.browser.executable)}
                    size={18}
                    alt={row.browser.display_name}
                  />
                </span>
                <span className="profile-order-browser-copy">
                  <span className="profile-order-browser-name">
                    {row.browser.display_name}
                  </span>
                  <span className="profile-order-browser-meta">
                    {row.error
                      ? t("profileOrder.scanFailed")
                      : t("profileOrder.profileCount", {
                          count: row.profiles.length,
                        })}
                  </span>
                </span>
                {hiddenCount > 0 && (
                  <span
                    className="profile-order-hidden-chip"
                    aria-label={t("profileOrder.hiddenTitle", {
                      count: hiddenCount,
                    })}
                  >
                    {t("profileOrder.newChip", { count: hiddenCount })}
                  </span>
                )}
                <span className="profile-order-browser-status">
                  {saved
                    ? t("profileOrder.statusSaved")
                    : canConfigure
                      ? t("profileOrder.statusConfigure")
                      : t("profileOrder.statusUnavailable")}
                </span>
              </button>
            );
          })}
        </div>

        {configurableCount === 0 && (
          <div className="profile-order-empty">
            {t("profileOrder.noneConfigurable")}
          </div>
        )}
        {error && <div className="profile-order-error">{error}</div>}
      </div>

      {editingRow && (
        <ProfileOrderDialog
          row={editingRow}
          savedOrder={doc?.settings.profile_orders?.[editingRow.browser.id]}
          pickerStyle={pickerStyle}
          onClose={() => setEditingBrowserId(null)}
          onSaved={onConfigChanged}
        />
      )}
    </Dialog.Root>
  );
}

function ProfileOrderDialog({
  row,
  savedOrder,
  pickerStyle,
  onClose,
  onSaved,
}: {
  row: BrowserProfileCatalog;
  savedOrder: string[] | undefined;
  pickerStyle: PickerStyle;
  onClose: () => void;
  onSaved: () => Promise<void> | void;
}) {
  const { t } = useTranslation("picker");
  const initial = useMemo(
    () => initialDraftFromSaved(row.profiles, savedOrder),
    [row.profiles, savedOrder],
  );
  const [draft, setDraft] = useState<BrowserProfile[]>(initial.profiles);
  const [selectedId, setSelectedId] = useState<string | null>(
    initial.profiles[0]?.id ?? null,
  );
  const [profilePickerOpen, setProfilePickerOpen] = useState(false);
  const [clearOnSave, setClearOnSave] = useState(initial.clearOnSave);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(initial.profiles);
    setSelectedId(initial.profiles[0]?.id ?? null);
    setClearOnSave(initial.clearOnSave);
    setProfilePickerOpen(false);
    setError(null);
  }, [initial]);

  const selectedIndex = useMemo(() => {
    const idx = draft.findIndex((profile) => profile.id === selectedId);
    return idx >= 0 ? idx : 0;
  }, [draft, selectedId]);
  const selectedProfile = draft[selectedIndex] ?? null;
  const hiddenProfiles = useMemo(() => {
    const visible = new Set(draft.map((profile) => profile.id));
    return sortProfilesDefault(
      row.profiles.filter((profile) => !visible.has(profile.id)),
    );
  }, [draft, row.profiles]);

  useEffect(() => {
    if (hiddenProfiles.length === 0 && profilePickerOpen) {
      setProfilePickerOpen(false);
    }
  }, [hiddenProfiles.length, profilePickerOpen]);

  const removeSelected = useCallback(() => {
    if (draft.length <= 1) return;
    const next = draft.filter((_, idx) => idx !== selectedIndex);
    setDraft(next);
    setSelectedId(next[Math.min(selectedIndex, next.length - 1)]?.id ?? null);
    setProfilePickerOpen(false);
    setClearOnSave(false);
  }, [draft, selectedIndex]);

  const addProfile = useCallback(
    (profileId: string) => {
      const profile = row.profiles.find((p) => p.id === profileId);
      if (!profile) return;
      const next = [...draft, profile];
      setDraft(next);
      setSelectedId(profile.id);
      setProfilePickerOpen(false);
      setClearOnSave(false);
    },
    [draft, row.profiles],
  );

  const reorderProfile = useCallback(
    (fromIndex: number, toIndex: number) => {
      if (fromIndex === toIndex) return;
      if (fromIndex < 0 || fromIndex >= draft.length) return;
      const boundedTo = Math.min(Math.max(toIndex, 0), draft.length - 1);
      const next = [...draft];
      const [moved] = next.splice(fromIndex, 1);
      if (!moved) return;
      next.splice(boundedTo, 0, moved);
      setDraft(next);
      setSelectedId(moved.id);
      setProfilePickerOpen(false);
      setClearOnSave(false);
    },
    [draft],
  );

  const selectHaloSlot = useCallback(
    (idx: number) => {
      if (idx === draft.length && hiddenProfiles.length > 0) {
        setProfilePickerOpen(true);
        return;
      }
      const profile = draft[idx];
      if (!profile) return;
      setSelectedId(profile.id);
      setProfilePickerOpen(false);
    },
    [draft, hiddenProfiles.length],
  );

  const restoreDefaultDraft = useCallback(() => {
    const next = sortProfilesDefault(row.profiles);
    setDraft(next);
    setSelectedId(next[0]?.id ?? null);
    setClearOnSave(true);
    setProfilePickerOpen(false);
    setError(null);
  }, [row.profiles]);

  const save = useCallback(async () => {
    if (draft.length < 1) {
      setError(t("profileOrder.keepOne"));
      return;
    }
    setPending(true);
    setError(null);
    try {
      await ipc.setProfileOrder(
        row.browser.id,
        clearOnSave ? [] : draft.map((profile) => profile.id),
      );
      await onSaved();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }, [clearOnSave, draft, onClose, onSaved, row.browser.id, t]);

  const previewProfiles = useMemo(
    () => draft.map(toPickerProfile),
    [draft],
  );
  const hasAddSlot = hiddenProfiles.length > 0;
  const canRemove = draft.length > 1 && selectedProfile !== null;
  const deletePosition = selectedProfile
    ? deleteControlPosition(
        selectedIndex,
        draft.length + (hasAddSlot ? 1 : 0),
        EDITOR_HALO_SIZE,
      )
    : null;

  return (
    <Dialog.Portal>
      <Dialog.Overlay className="profile-order-modal-overlay" />
      <Dialog.Content className="profile-order-modal">
        <div className="profile-order-modal-head">
          <div>
            <Dialog.Title className="profile-order-modal-title">
              {t("profileOrder.dialogTitle")}
            </Dialog.Title>
            <Dialog.Description className="profile-order-modal-desc">
              {t("profileOrder.dialogDescription", {
                browser: row.browser.display_name,
                visible: draft.length,
                detected: row.profiles.length,
              })}
            </Dialog.Description>
          </div>
          <Dialog.Close asChild>
            <button
              type="button"
              className="profile-order-icon-btn"
              aria-label={t("profileOrder.close")}
            >
              <X size={16} />
            </button>
          </Dialog.Close>
        </div>

        <div className="profile-order-modal-body">
          <div className="profile-order-halo-stage">
            {draft.length > 0 ? (
              <>
                <div className="profile-order-wheel-wrap">
                  <HaloPreview
                    style={pickerStyle}
                    size={EDITOR_HALO_SIZE}
                    profiles={previewProfiles}
                    activeIndex={selectedIndex}
                    selectedIndex={selectedIndex}
                    interactive
                    addSlot={hasAddSlot}
                    draggable
                    onSelectIndex={selectHaloSlot}
                    onReorder={reorderProfile}
                  />

                  {deletePosition && (
                    <button
                      type="button"
                      className="profile-order-sector-delete"
                      style={{
                        left: deletePosition.left,
                        top: deletePosition.top,
                      }}
                      disabled={!canRemove}
                      onClick={removeSelected}
                      title={
                        canRemove
                          ? t("profileOrder.hideSelected")
                          : t("profileOrder.mustRemain")
                      }
                      aria-label={t("profileOrder.hideSelected")}
                    >
                      <Trash2 size={16} />
                    </button>
                  )}
                </div>

                {selectedProfile && (
                  <div className="profile-order-selected-chip">
                    <span
                      className="profile-order-avatar"
                      style={{
                        background:
                          selectedProfile.accent_color ??
                          FALLBACK_PALETTE[
                            selectedIndex % FALLBACK_PALETTE.length
                          ],
                      }}
                    >
                      {profileMonogram(toPickerProfile(selectedProfile))}
                    </span>
                    <span className="profile-order-selected-copy">
                      <span>{selectedProfile.display_name}</span>
                      {selectedProfile.email && (
                        <span>{selectedProfile.email}</span>
                      )}
                    </span>
                  </div>
                )}

                {profilePickerOpen && hiddenProfiles.length > 0 && (
                  <ProfileChooserOverlay
                    row={row}
                    profiles={hiddenProfiles}
                    onClose={() => setProfilePickerOpen(false)}
                    onPick={addProfile}
                  />
                )}
              </>
            ) : (
              <div className="profile-order-halo-empty">
                {t("profileOrder.haloEmpty")}
              </div>
            )}
          </div>

          {(clearOnSave || error) && (
            <div
              className={error ? "profile-order-error" : "profile-order-note"}
            >
              {error ??
                t("profileOrder.clearNote")}
            </div>
          )}
        </div>

        <div className="profile-order-modal-footer">
          <button
            type="button"
            className="mac-tbtn"
            onClick={restoreDefaultDraft}
            disabled={pending}
          >
            <RotateCcw size={14} />
            {t("profileOrder.reset")}
          </button>
          <span className="grow" />
          <Dialog.Close asChild>
            <button type="button" className="mac-tbtn" disabled={pending}>
              {t("profileOrder.cancel")}
            </button>
          </Dialog.Close>
          <button
            type="button"
            className="mac-tbtn primary"
            disabled={pending || draft.length < 1}
            onClick={() => void save()}
          >
            {pending ? t("profileOrder.saving") : t("profileOrder.save")}
          </button>
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  );
}

function ProfileChooserOverlay({
  row,
  profiles,
  onClose,
  onPick,
}: {
  row: BrowserProfileCatalog;
  profiles: BrowserProfile[];
  onClose: () => void;
  onPick: (profileId: string) => void;
}) {
  const { t } = useTranslation("picker");
  return (
    <div
      className="profile-order-profile-picker"
      role="dialog"
      aria-label={t("profileOrder.chooseProfileAria")}
    >
      <div className="profile-order-profile-picker-head">
        <span className="profile-order-profile-picker-app">
          <AppIcon
            bundleId={row.browser.platform_app_id ?? undefined}
            appPath={appPathFromExecutable(row.browser.executable)}
            size={24}
            alt={row.browser.display_name}
          />
        </span>
        <span className="profile-order-profile-picker-title">
          <span>{t("profileOrder.chooseProfile")}</span>
          <span>{row.browser.display_name}</span>
        </span>
        <button
          type="button"
          className="profile-order-icon-btn"
          onClick={onClose}
          aria-label={t("profileOrder.closeChooser")}
        >
          <X size={16} />
        </button>
      </div>

      <div className="profile-order-profile-grid">
        {profiles.map((profile, idx) => (
          <button
            key={profile.id}
            type="button"
            className="profile-order-profile-card"
            onClick={() => onPick(profile.id)}
          >
            <span
              className="profile-order-profile-avatar"
              style={{
                background:
                  profile.accent_color ??
                  FALLBACK_PALETTE[idx % FALLBACK_PALETTE.length],
              }}
            >
              {profileMonogram(toPickerProfile(profile))}
            </span>
            <span className="profile-order-profile-card-copy">
              <span>{profile.display_name}</span>
              {profile.email && <span>{profile.email}</span>}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

/** Render a list of browser names as "A", "A and B", or "A, B and C".
 *  Kept inline because Intl.ListFormat isn't worth a runtime config dep
 *  for this single call site. */
function formatBrowserList(t: TFunction<"picker">, names: string[]): string {
  if (names.length === 0) return "";
  if (names.length === 1) return names[0];
  if (names.length === 2) return `${names[0]} ${t("profileOrder.and")} ${names[1]}`;
  return `${names.slice(0, -1).join(", ")} ${t("profileOrder.and")} ${
    names[names.length - 1]
  }`;
}

function hasSavedOrder(doc: ConfigDocument | null, browserId: string): boolean {
  const saved = doc?.settings.profile_orders?.[browserId];
  return saved != null && saved.length > 0;
}

function initialDraftFromSaved(
  raw: BrowserProfile[],
  saved: string[] | undefined,
): InitialDraft {
  if (!saved || saved.length === 0) {
    return { profiles: sortProfilesDefault(raw), clearOnSave: true };
  }

  const byId = new Map(raw.map((profile) => [profile.id, profile]));
  const profiles: BrowserProfile[] = [];
  for (const id of saved) {
    const profile = byId.get(id);
    if (profile) {
      profiles.push(profile);
      byId.delete(id);
    }
  }

  return profiles.length >= 2
    ? { profiles, clearOnSave: false }
    : { profiles: sortProfilesDefault(raw), clearOnSave: true };
}

function sortProfilesDefault(raw: BrowserProfile[]): BrowserProfile[] {
  return [...raw].sort(
    (a, b) =>
      Number(!!b.is_default) - Number(!!a.is_default) ||
      a.display_name.localeCompare(b.display_name),
  );
}

function toPickerProfile(profile: BrowserProfile): PickerProfile {
  return {
    id: profile.id,
    name: profile.display_name,
    email: profile.email,
    accent_color: profile.accent_color,
    is_default: !!profile.is_default,
  };
}

function deleteControlPosition(
  index: number,
  slotCount: number,
  size: number,
): { left: number; top: number } {
  const angle = sectorMidAngle(index, slotCount);
  const [left, top] = polarToCart(size / 2, size / 2, size * 0.49, angle);
  return { left, top };
}
