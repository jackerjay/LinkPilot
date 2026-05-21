// HaloShell — the parts shared by every Halo variant:
//   • ⌥ modifier tracking
//   • hovered tile + hovered profile state
//   • tile bounding-box tracking (so portals can anchor to the tile)
//   • keyboard handlers (1-9 launch, Enter = default, Esc = cancel)
//   • release ⌥ over a hovered sector to launch without an extra click
//   • click-zone-scoped launch (clicks outside the wheel never fire)
//   • portal-rendered profile readout above the wheel
//
// Each variant only has to supply `renderPortal(args)` to paint its sectors
// + decorations inside the portal — geometry is identical across variants.
//
// Why a hook + render-prop instead of a class: the design's `HaloShell`
// component used render-prop too, and it keeps each variant tiny (~80 lines
// of paint code each) without React.Context plumbing.

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { AppIcon } from "@/components/AppIcon";
import {
  DEFAULT_HALO_GEOMETRY,
  type HaloGeometry,
  type PickerChoice,
  type PickerProfile,
  profileAccent,
  SUMMON_KEY_LABEL,
} from "./types";

interface LaunchEvent {
  browser: PickerChoice;
  profile: PickerProfile;
}

interface PortalArgs {
  /** Bounding box (in viewport coords) of the armed browser tile, used to
   *  anchor the wheel + readout. */
  tileRect: DOMRect;
  browser: PickerChoice;
  hoveredProfile: number | null;
  geometry: HaloGeometry;
}

function isSummonKeyRelease(e: KeyboardEvent): boolean {
  return (
    e.key === "Alt" ||
    e.code === "AltLeft" ||
    e.code === "AltRight" ||
    !e.altKey
  );
}

interface PointerPoint {
  x: number;
  y: number;
}

function profileIndexFromPoint(
  point: PointerPoint,
  tileRect: DOMRect,
  profileCount: number,
  geometry: HaloGeometry,
): number | null {
  const cx = tileRect.left + tileRect.width / 2;
  const cy = tileRect.top + tileRect.height / 2;
  const dx = point.x - cx;
  const dy = point.y - cy;
  const dist = Math.hypot(dx, dy);
  const innerR = geometry.innerRadius - 22;
  const outerR = geometry.outerRadius + geometry.hoverPush + 60;
  if (dist < innerR || dist > outerR) return null;

  const ang = Math.atan2(dy, dx);
  const baseline = -Math.PI / 2 - Math.PI / profileCount;
  let t = (ang - baseline) / (Math.PI * 2);
  t = ((t % 1) + 1) % 1;
  return Math.floor(t * profileCount);
}

export interface HaloShellProps {
  choices: PickerChoice[];
  url: string;
  onPick: (browserId: string, profileId: string | null) => void;
  onCancel: () => void;
  renderPortal: (args: PortalArgs) => React.ReactNode;
  /** Crown owns its own readout (center display) so it suppresses the
   *  portal-rendered card. Defaults to `true`. */
  showReadout?: boolean;
  /** Override geometry. Production uses `DEFAULT_HALO_GEOMETRY`; tests can
   *  inject a smaller wheel. */
  geometry?: HaloGeometry;
}

function useModKey(): boolean {
  const [down, setDown] = useState(false);
  useEffect(() => {
    const onDown = (e: KeyboardEvent) => {
      if (e.altKey) setDown(true);
    };
    const onUp = (e: KeyboardEvent) => {
      if (!e.altKey) setDown(false);
    };
    const onBlur = () => setDown(false);
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onDown);
      window.removeEventListener("keyup", onUp);
      window.removeEventListener("blur", onBlur);
    };
  }, []);
  return down;
}

export function HaloShell({
  choices,
  url,
  onPick,
  onCancel,
  renderPortal,
  showReadout = true,
  geometry = DEFAULT_HALO_GEOMETRY,
}: HaloShellProps) {
  const { t } = useTranslation("picker");
  const optDown = useModKey();
  const tileRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const [hoveredTileIdx, setHoveredTileIdx] = useState<number | null>(null);
  const [hoveredProfile, setHoveredProfile] = useState<number | null>(null);
  const [tileRect, setTileRect] = useState<DOMRect | null>(null);
  const [launching, setLaunching] = useState<LaunchEvent | null>(null);
  const launchStartedRef = useRef(false);
  const lastPointerRef = useRef<PointerPoint | null>(null);

  const browser = hoveredTileIdx != null ? choices[hoveredTileIdx] : null;
  const showWheel =
    optDown &&
    browser != null &&
    browser.profiles.length > 1 &&
    tileRect != null &&
    launching == null;
  const releaseTargetRef = useRef<{
    browser: PickerChoice | null;
    hoveredProfile: number | null;
    showWheel: boolean;
    tileRect: DOMRect | null;
  }>({
    browser: null,
    hoveredProfile: null,
    showWheel: false,
    tileRect: null,
  });
  releaseTargetRef.current = {
    browser,
    hoveredProfile,
    showWheel,
    tileRect,
  };

  // Track the armed tile's bounding rect. Needed because the wheel is
  // portaled to document.body and has to know where the tile is in
  // viewport coords. The wheel's portal receives normal mouse events only to
  // keep the transparent Tauri window out of drag-region mode; hit-testing
  // still reads coords through a window-level listener.
  useEffect(() => {
    const multi = browser != null && browser.profiles.length > 1;
    if (!multi || !browser) {
      setTileRect(null);
      return;
    }
    const measure = () => {
      const el = tileRefs.current[browser.id];
      if (el) setTileRect(el.getBoundingClientRect());
    };
    measure();
    window.addEventListener("scroll", measure, true);
    window.addEventListener("resize", measure);
    return () => {
      window.removeEventListener("scroll", measure, true);
      window.removeEventListener("resize", measure);
    };
  }, [browser?.id, browser?.profiles.length, browser]);

  // Mouse-direction hit-test (the wheel uses "aim" not "click target" —
  // pointer direction relative to the wheel center decides which sector is
  // hovered, regardless of distance within the annulus). Implemented as a
  // window-level listener so it works inside the portal-painted wheel, whose
  // surface is marked no-drag to keep WebView mouse events flowing.
  useEffect(() => {
    if (!showWheel || !tileRect || !browser) {
      setHoveredProfile(null);
      return;
    }
    const onMove = (e: MouseEvent) => {
      const point = { x: e.clientX, y: e.clientY };
      lastPointerRef.current = point;
      setHoveredProfile(
        profileIndexFromPoint(point, tileRect, browser.profiles.length, geometry),
      );
    };
    window.addEventListener("mousemove", onMove);
    return () => window.removeEventListener("mousemove", onMove);
  }, [
    showWheel,
    tileRect,
    browser,
    geometry.innerRadius,
    geometry.outerRadius,
    geometry.hoverPush,
  ]);

  const launch = useCallback(
    (b: PickerChoice, p: PickerProfile) => {
      if (launchStartedRef.current) return;
      launchStartedRef.current = true;
      setLaunching({ browser: b, profile: p });
      // Show the toast briefly before resolving the pick — gives the user a
      // last-glance "yes, this is what I picked". The Tauri side closes the
      // window as soon as `picker_resolve` fires, so the toast lifetime is
      // bounded by the round-trip + animation tail.
      setTimeout(() => onPick(b.id, p.id), 280);
    },
    [onPick],
  );

  // Keyboard:
  //   1-9            → launch profile at that index (uses e.code to dodge
  //                    macOS's ⌥+digit = ¡ remap)
  //   Enter / Space  → launch hovered profile, or default if none hovered
  //   Esc            → cancel
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (launching) {
          setLaunching(null);
          return;
        }
        if (hoveredProfile != null) {
          setHoveredProfile(null);
          return;
        }
        e.preventDefault();
        onCancel();
        return;
      }
      if (!showWheel || !browser) return;
      const digit = /^Digit([1-9])$/.exec(e.code);
      if (digit) {
        const target = parseInt(digit[1], 10) - 1;
        if (target < browser.profiles.length) {
          e.preventDefault();
          launch(browser, browser.profiles[target]);
        }
        return;
      }
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        const idx = hoveredProfile != null ? hoveredProfile : 0;
        launch(browser, browser.profiles[idx]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showWheel, browser, hoveredProfile, launching, launch, onCancel]);

  // Click-to-launch — scoped to the wheel's hit zone (same annulus used by
  // the hover hit-test). Stops the click from bubbling so it doesn't also
  // count as "clicked the tile" or "clicked the popover".
  useEffect(() => {
    if (!showWheel || !tileRect || !browser) return;
    const cx = tileRect.left + tileRect.width / 2;
    const cy = tileRect.top + tileRect.height / 2;
    const innerR = geometry.innerRadius - 22;
    const outerR = geometry.outerRadius + geometry.hoverPush + 60;
    const onClick = (e: MouseEvent) => {
      const dist = Math.hypot(e.clientX - cx, e.clientY - cy);
      if (dist < innerR || dist > outerR) return;
      if (hoveredProfile == null) return;
      e.preventDefault();
      e.stopPropagation();
      launch(browser, browser.profiles[hoveredProfile]);
    };
    window.addEventListener("click", onClick, true);
    return () => window.removeEventListener("click", onClick, true);
  }, [
    showWheel,
    hoveredProfile,
    browser,
    tileRect,
    launch,
    geometry.innerRadius,
    geometry.outerRadius,
    geometry.hoverPush,
  ]);

  // Release-to-launch — once the wheel is open, the mouse's current sector
  // is already the user's target. Keep this listener mounted and read refs
  // at keyup time: modifier keyup and React state cleanup happen in the same
  // turn, so conditional listener mounting can miss the final release.
  useEffect(() => {
    const onKeyUp = (e: KeyboardEvent) => {
      if (!isSummonKeyRelease(e)) return;
      const target = releaseTargetRef.current;
      if (
        !target.showWheel ||
        !target.browser ||
        !target.tileRect ||
        launchStartedRef.current
      ) {
        return;
      }
      const aimedProfile =
        lastPointerRef.current != null
          ? profileIndexFromPoint(
              lastPointerRef.current,
              target.tileRect,
              target.browser.profiles.length,
              geometry,
            )
          : target.hoveredProfile;
      if (aimedProfile == null) return;
      e.preventDefault();
      launch(target.browser, target.browser.profiles[aimedProfile]);
    };
    window.addEventListener("keyup", onKeyUp, true);
    return () => window.removeEventListener("keyup", onKeyUp, true);
  }, [geometry, launch]);

  const urlPreview =
    url.length > 80 ? url.slice(0, 80) + "…" : url;

  return (
    <div className={`pk-pop${launching ? " launching" : ""}`}>
      {launching && <LaunchToast event={launching} />}

      <div className="pk-pop-head">
        <div className="pk-eyebrow">{t("halo.openWith")}</div>
        <div className="pk-url" title={url}>
          {urlPreview}
        </div>
      </div>

      <div className="wh-b-stage">
        {/* ⌥ pill — sits centered, lights up when the modifier is held */}
        {choices.some((c) => c.profiles.length > 1) && (
          <ModifierPill active={optDown} />
        )}

        <div className="wh-b-tile-row">
          {choices.map((c, idx) => {
            const multi = c.profiles.length > 1;
            return (
              <button
                key={c.id}
                type="button"
                ref={(el) => {
                  tileRefs.current[c.id] = el;
                }}
                className={
                  "pk-tile" +
                  (optDown && multi ? " armed" : "") +
                  (showWheel && idx === hoveredTileIdx ? " wheel-anchor" : "") +
                  (showWheel && idx !== hoveredTileIdx ? " dim" : "")
                }
                style={{
                  appearance: "none",
                  font: "inherit",
                  color: "inherit",
                  outline: "none",
                }}
                onMouseEnter={(e) => {
                  lastPointerRef.current = {
                    x: e.clientX,
                    y: e.clientY,
                  };
                  setHoveredTileIdx(idx);
                  setHoveredProfile(null);
                }}
                onClick={() => {
                  // Plain click on a tile (no ⌥) → launch the default
                  // profile. Same behaviour as single-profile browsers.
                  if (!optDown || !multi) {
                    const defaultProfile =
                      c.profiles.find(
                        (p) => p.id === c.default_profile_id,
                      ) ??
                      c.profiles[0] ??
                      null;
                    if (defaultProfile) {
                      launch(c, defaultProfile);
                    } else {
                      // Browser with zero known profiles (Safari etc.).
                      // Resolve without a profile id; the backend
                      // applies the user's rule default.
                      setLaunching({
                        browser: c,
                        profile: { id: "", name: c.name, is_default: true },
                      });
                      setTimeout(() => onPick(c.id, null), 280);
                    }
                  }
                }}
              >
                <div className="pk-tile-icon">
                  {c.icon_data_url ? (
                    <img
                      src={c.icon_data_url}
                      alt={c.name}
                      width={60}
                      height={60}
                    />
                  ) : (
                    <AppIcon
                      bundleId={c.bundle_id ?? undefined}
                      appPath={c.app_path ?? undefined}
                      name={c.name}
                      size={60}
                      alt={c.name}
                      className="rounded-xl"
                    />
                  )}
                </div>
                <div className="pk-tile-name">{c.name}</div>
                <div className="pk-b-tile-count">
                  {multi
                    ? t("halo.profileCount", { count: c.profiles.length })
                    : " "}
                </div>
              </button>
            );
          })}
        </div>
      </div>

      <div className="pk-pop-foot">
        <span className="pk-foot-group">
          <span className="pk-kbd">{SUMMON_KEY_LABEL}</span>{" "}
          {t("halo.footer.summon")}
        </span>
        <span className="pk-foot-group">
          <span className="pk-kbd">1</span>
          <span className="pk-kbd">9</span> {t("halo.footer.direct")}
        </span>
        <span className="pk-foot-group">
          <span className="pk-kbd">⏎</span> {t("halo.footer.default")}
        </span>
        <span className="pk-foot-group">
          <span className="pk-kbd">ESC</span> {t("halo.footer.cancel")}
        </span>
      </div>

      {showWheel && browser && tileRect && (
        <>
          {renderPortal({
            tileRect,
            browser,
            hoveredProfile,
            geometry,
          })}
          {showReadout && (
            <HaloReadoutPortal
              tileRect={tileRect}
              browser={browser}
              profile={
                hoveredProfile != null
                  ? browser.profiles[hoveredProfile]
                  : null
              }
              profileIdx={hoveredProfile}
            />
          )}
        </>
      )}
    </div>
  );
}

// -------- ⌥ pill --------------------------------------------------------

function ModifierPill({ active }: { active: boolean }) {
  const { t } = useTranslation("picker");
  return (
    <div className={"pk-b-modkey" + (active ? " active" : "")}>
      <span className="pk-b-modkey-kbd">{SUMMON_KEY_LABEL}</span>
      <span>
        {active ? t("halo.modifierActive") : t("halo.modifierIdle")}
      </span>
    </div>
  );
}

// -------- Readout portal ------------------------------------------------

function HaloReadoutPortal({
  tileRect,
  browser,
  profile,
  profileIdx,
}: {
  tileRect: DOMRect;
  browser: PickerChoice;
  profile: PickerProfile | null;
  profileIdx: number | null;
}) {
  const { t } = useTranslation("picker");
  const cx = tileRect.left + tileRect.width / 2;
  const cy = tileRect.top + tileRect.height / 2;
  // Readout sits 200px above the wheel center. With the standard 152px
  // outer radius + breathing room that puts it clear of the sector ring.
  const offsetUp = 200;
  return createPortal(
    <div
      className="halo-readout-portal"
      style={{
        position: "fixed",
        left: cx,
        top: cy - offsetUp,
        transform: "translate(-50%, -100%)",
        pointerEvents: "none",
        zIndex: 10001,
      }}
    >
      <div className="halo-readout-card">
        {profile ? (
          <>
            <span className="halo-readout-browser">
              <span>{browser.name}</span>
            </span>
            <span
              className="halo-readout-name"
              style={{
                color:
                  profileIdx != null
                    ? profileAccent(profile, profileIdx)
                    : undefined,
              }}
            >
              {profile.name}
            </span>
            {profile.email && (
              <span className="halo-readout-email">{profile.email}</span>
            )}
          </>
        ) : (
          <span className="halo-readout-prompt">
            {browser.name} ·{" "}
            {t("halo.profileCount", { count: browser.profiles.length })} ·{" "}
            <span className="halo-readout-hint">
              {t("halo.readoutHint")}
            </span>
          </span>
        )}
      </div>
    </div>,
    document.body,
  );
}

// -------- Launch toast --------------------------------------------------

function LaunchToast({ event }: { event: LaunchEvent }) {
  const { t } = useTranslation("picker");
  const idx = event.browser.profiles.findIndex((p) => p.id === event.profile.id);
  const color = idx >= 0 ? profileAccent(event.profile, idx) : "#6E6E73";
  const letter = event.profile.name.trim().charAt(0).toUpperCase() || "•";
  return (
    <div className="wh-launch-toast">
      <span className="wh-launch-mark" style={{ background: color }}>
        {letter}
      </span>
      <div className="wh-launch-text">
        <span className="wh-launch-line1">
          {t("halo.opening", { browser: event.browser.name })}
        </span>
        <span className="wh-launch-line2">
          {event.profile.name}
          {event.profile.email && ` · ${event.profile.email}`}
        </span>
      </div>
    </div>
  );
}
