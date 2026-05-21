// Picker session shape mirrored from `src-tauri/src/picker.rs`. Kept in this
// folder rather than in `src/lib/types.ts` because nothing outside the picker
// renderer needs to know about these — they're surfaced exclusively via
// `picker_session` / `picker_resolve` Tauri commands.

import type { PickerStyle } from "@/lib/types";

export interface PickerProfile {
  id: string;
  name: string;
  email?: string | null;
  /** Hex `#RRGGBB`, deterministic per profile. Frontend treats `null` as
   *  "use the fallback palette index based on array position". */
  accent_color?: string | null;
  is_default: boolean;
}

export interface PickerChoice {
  id: string;
  name: string;
  bundle_id?: string | null;
  app_path?: string | null;
  icon_data_url?: string | null;
  /** Ordered and filtered by Settings profile order, or default-first
   *  alphabetical when no customization exists. */
  profiles: PickerProfile[];
  default_profile_id?: string | null;
}

export interface PickerSession {
  url: string;
  choices: PickerChoice[];
  /** Locked at picker-open time — the renderer reads this once and doesn't
   *  hot-reload if the user flips the Settings page mid-pick. */
  style: PickerStyle;
}

/** What the renderer hands back to Rust via `picker_resolve`. */
export interface PickerPick {
  browser_id: string;
  profile_id?: string | null;
}

/** Single source of truth for which modifier key arms the wheel. The design
 *  exploration considered ⌃ / ⇧ too — we expose only ⌥ for now because macOS
 *  conventions assign ⌥ to "show alternate action", which is exactly what
 *  the wheel surfaces. */
export const SUMMON_KEY_LABEL = "⌥";

/** Geometry knobs shared by all 3 variants. The design's Tweaks panel let
 *  users tune these live; in production they're fixed (anyone tinkering can
 *  edit this file). */
export interface HaloGeometry {
  /** Inner radius — Chrome tile sits inside this, sectors start here. */
  innerRadius: number;
  /** Outer radius of the sector ring (before hover push). */
  outerRadius: number;
  /** Extra pixels the hovered sector pushes outward. */
  hoverPush: number;
  /** Frosted-only: width of the colored band on the outer rim. */
  rimWidth: number;
  /** Whether to paint 1–9 number badges on the inner edge. */
  showNumbers: boolean;
}

export const DEFAULT_HALO_GEOMETRY: HaloGeometry = {
  innerRadius: 56,
  outerRadius: 152,
  hoverPush: 8,
  rimWidth: 4,
  showNumbers: true,
};

/** Fallback palette when a profile doesn't carry an accent_color (Safari,
 *  custom browsers we don't know how to inventory). Indexed by the
 *  profile's array position so the picker always paints something. */
export const FALLBACK_PALETTE = [
  "#4285F4",
  "#34A853",
  "#EA4335",
  "#FBBC04",
  "#9C27B0",
  "#FF6D00",
  "#00ACC1",
  "#43A047",
  "#5E35B1",
  "#6E6E73",
  "#E91E63",
  "#3F51B5",
];

export function profileAccent(p: PickerProfile, idx: number): string {
  return p.accent_color ?? FALLBACK_PALETTE[idx % FALLBACK_PALETTE.length];
}

/** Single-character monogram used inside the wheel sectors. Derived from
 *  the profile name (first uppercase glyph). Falls back to `#` if the name
 *  is empty / non-ASCII. */
export function profileMonogram(p: PickerProfile): string {
  const trimmed = p.name.trim();
  if (!trimmed) return "#";
  // Take first code point so we don't slice a surrogate pair in half.
  const ch = Array.from(trimmed)[0];
  return ch.toUpperCase();
}
