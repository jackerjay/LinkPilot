// Static, scaled-down preview of the three Halo variants for the Settings
// page. Inline SVG (no portal, no listeners, no DOMRect math) so it can
// safely render multiple instances side-by-side in a card grid.
//
// Why duplicate the production portals' paint code instead of refactoring
// them to support inline mode: the production paths need viewport-anchored
// fixed positioning + the frost-disc backdrop + the ⌥-armed mouse
// hit-test scaffolding. A preview wants none of that. Keeping the two
// paths separate means changes to the production picker can't accidentally
// break the Settings preview, and vice versa.

import {
  polarToCart,
  sectorBoundaryAngle,
  sectorMidAngle,
  sectorPath,
} from "./geometry";
import { FALLBACK_PALETTE, profileMonogram } from "./types";
import type { PickerStyle } from "@/lib/types";

// Mock profile set — picked to show off both density and variety. Five
// entries strikes a balance: enough to demonstrate the wheel's geometry,
// few enough to keep the preview legible at 160px.
const MOCK_PROFILES = [
  { id: "default", name: "Default", email: "you@gmail.com", is_default: true },
  { id: "work", name: "Work", email: "you@company.example", is_default: false },
  { id: "side", name: "Side", email: "side@me.com", is_default: false },
  { id: "oss", name: "OSS", email: "you@oss.dev", is_default: false },
  { id: "personal", name: "Personal", email: "you@me.com", is_default: false },
];

// Index of the "hovered" profile in the preview — fixed so previews are
// deterministic. We chose 1 ("Work") because it shows off the colored
// hover state without being the default profile (which would look the same
// as the idle Crown state, defeating the point of the preview).
const HOVERED_IDX = 1;

interface HaloPreviewProps {
  style: PickerStyle;
  /** Render width in CSS pixels. The viewport is square. Default 160. */
  size?: number;
}

export function HaloPreview({ style, size = 160 }: HaloPreviewProps) {
  // Scaled-down geometry. The ratios mirror the production picker:
  //   ri ≈ 0.15 * W, ro ≈ 0.38 * W, rim ≈ 3px
  // so the preview is visually faithful even at this size.
  const W = size;
  const half = W / 2;

  return (
    <div
      style={{
        position: "relative",
        width: W,
        height: W,
        borderRadius: "50%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {/* Frosted backdrop disc, matching production's `halo-frost-disc`. */}
      <div
        className={`halo-frost-disc${style === "bezel" ? " subtle" : ""}${
          style === "crown" ? " strong" : ""
        }`}
        style={{
          position: "absolute",
          inset: 0,
          width: W,
          height: W,
        }}
      />

      <svg
        width={W}
        height={W}
        viewBox={`0 0 ${W} ${W}`}
        style={{ position: "absolute", inset: 0, overflow: "visible" }}
      >
        {style === "frosted" && <FrostedSectors W={W} />}
        {style === "bezel" && <BezelSectors W={W} />}
        {style === "crown" && <CrownSectors W={W} />}
      </svg>

      {style === "crown" && <CrownCenter W={W} half={half} />}
    </div>
  );
}

// ---------- α Frosted -----------------------------------------------------

function FrostedSectors({ W }: { W: number }) {
  const half = W / 2;
  const ri = W * 0.18;
  const ro = W * 0.42;
  const rimRest = 2;
  const rimHover = 4;
  const hoverPush = 3;
  const n = MOCK_PROFILES.length;
  return (
    <>
      {MOCK_PROFILES.map((p, i) => {
        const halfA = Math.PI / n;
        const mid = sectorMidAngle(i, n);
        const a1 = mid - halfA + 0.012;
        const a2 = mid + halfA - 0.012;
        const isH = i === HOVERED_IDX;
        const ro_ = ro + (isH ? hoverPush : 0);
        const rimW = isH ? rimHover : rimRest;
        const accent = FALLBACK_PALETTE[i % FALLBACK_PALETTE.length];
        return (
          <g key={p.id}>
            <path
              d={sectorPath(half, half, ri, ro_, a1, a2)}
              fill={isH ? "rgba(255,255,255,0.24)" : "rgba(255,255,255,0.04)"}
            />
            <path
              d={sectorPath(half, half, ro_ - rimW, ro_, a1, a2)}
              fill={accent}
              opacity={isH ? 0.74 : 0.42}
            />
          </g>
        );
      })}
      {/* Inter-sector hairlines */}
      {MOCK_PROFILES.map((p, i) => {
        const mid = sectorBoundaryAngle(i, n);
        const [x1, y1] = polarToCart(half, half, ri, mid);
        const [x2, y2] = polarToCart(half, half, ro, mid);
        return (
          <line
            key={`hr${p.id}`}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="rgba(0,0,0,0.08)"
            strokeWidth="0.5"
          />
        );
      })}
      {/* Letter monograms */}
      {MOCK_PROFILES.map((p, i) => {
        const mid = sectorMidAngle(i, n);
        const labR = ri + (ro - ri) * 0.48;
        const [lx, ly] = polarToCart(half, half, labR, mid);
        const isH = i === HOVERED_IDX;
        const accent = FALLBACK_PALETTE[i % FALLBACK_PALETTE.length];
        return (
          <text
            key={`lb${p.id}`}
            x={lx}
            y={ly}
            textAnchor="middle"
            dominantBaseline="central"
            fill={isH ? accent : "rgba(28,28,32,0.7)"}
            fontWeight={isH ? 600 : 500}
            fontSize={9}
            style={{
              fontFamily:
                'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
            }}
          >
            {profileMonogram(p)}
          </text>
        );
      })}
      <circle
        cx={half}
        cy={half}
        r={ri}
        fill="none"
        stroke="rgba(0,0,0,0.08)"
        strokeWidth="0.5"
      />
      <circle
        cx={half}
        cy={half}
        r={ro}
        fill="none"
        stroke="rgba(0,0,0,0.08)"
        strokeWidth="0.5"
      />
    </>
  );
}

// ---------- β Bezel -------------------------------------------------------

function BezelSectors({ W }: { W: number }) {
  const half = W / 2;
  const ri = W * 0.18;
  const ro = W * 0.42;
  const dotR = ri + (ro - ri) * 0.55;
  const labelR = ro + 8;
  const n = MOCK_PROFILES.length;
  return (
    <>
      {/* Hovered wedge fill */}
      {(() => {
        const i = HOVERED_IDX;
        const halfA = Math.PI / n;
        const mid = sectorMidAngle(i, n);
        const a1 = mid - halfA + 0.006;
        const a2 = mid + halfA - 0.006;
        return (
          <path
            d={sectorPath(half, half, ri, ro, a1, a2)}
            fill={FALLBACK_PALETTE[i % FALLBACK_PALETTE.length]}
            opacity="0.14"
          />
        );
      })()}
      {/* Bezel rings */}
      <circle
        cx={half}
        cy={half}
        r={ro}
        fill="none"
        stroke="rgba(0,0,0,0.14)"
        strokeWidth="1"
      />
      <circle
        cx={half}
        cy={half}
        r={ri}
        fill="none"
        stroke="rgba(0,0,0,0.08)"
        strokeWidth="0.5"
      />
      {/* Tick marks */}
      {MOCK_PROFILES.map((_, i) => {
        const mid = sectorBoundaryAngle(i, n);
        const [x1, y1] = polarToCart(half, half, ro - 5, mid);
        const [x2, y2] = polarToCart(half, half, ro, mid);
        return (
          <line
            key={`tk${i}`}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="rgba(0,0,0,0.30)"
            strokeWidth="1"
          />
        );
      })}
      {/* Profile dots */}
      {MOCK_PROFILES.map((p, i) => {
        const mid = sectorMidAngle(i, n);
        const [dx, dy] = polarToCart(half, half, dotR, mid);
        const isH = i === HOVERED_IDX;
        return (
          <circle
            key={`dt${p.id}`}
            cx={dx}
            cy={dy}
            r={isH ? 6 : 4}
            fill={FALLBACK_PALETTE[i % FALLBACK_PALETTE.length]}
            opacity={isH ? 1 : 0.78}
          />
        );
      })}
      {/* Letters outside the ring */}
      {MOCK_PROFILES.map((p, i) => {
        const mid = sectorMidAngle(i, n);
        const [lx, ly] = polarToCart(half, half, labelR, mid);
        const isH = i === HOVERED_IDX;
        const accent = FALLBACK_PALETTE[i % FALLBACK_PALETTE.length];
        return (
          <text
            key={`lb${p.id}`}
            x={lx}
            y={ly}
            textAnchor="middle"
            dominantBaseline="central"
            fill={isH ? accent : "rgba(28,28,32,0.78)"}
            fontWeight={isH ? 700 : 500}
            fontSize={8}
            style={{
              fontFamily:
                'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
            }}
          >
            {profileMonogram(p)}
          </text>
        );
      })}
    </>
  );
}

// ---------- γ Crown -------------------------------------------------------

function CrownSectors({ W }: { W: number }) {
  const half = W / 2;
  // Crown widens the inner radius — center display lives inside.
  const ri = W * 0.27;
  const ro = W * 0.44;
  const n = MOCK_PROFILES.length;
  return (
    <>
      {MOCK_PROFILES.map((p, i) => {
        const halfA = Math.PI / n;
        const mid = sectorMidAngle(i, n);
        const a1 = mid - halfA + 0.01;
        const a2 = mid + halfA - 0.01;
        const isH = i === HOVERED_IDX;
        const restFill =
          i % 2 === 0 ? "rgba(255,255,255,0.10)" : "rgba(255,255,255,0.06)";
        return (
          <g key={p.id}>
            <path
              d={sectorPath(half, half, ri, ro, a1, a2)}
              fill={isH ? "rgba(255,255,255,0.28)" : restFill}
            />
            {isH && (
              <path
                d={sectorPath(half, half, ro - 3, ro, a1, a2)}
                fill={FALLBACK_PALETTE[i % FALLBACK_PALETTE.length]}
              />
            )}
          </g>
        );
      })}
      {MOCK_PROFILES.map((p, i) => {
        const mid = sectorBoundaryAngle(i, n);
        const [x1, y1] = polarToCart(half, half, ri, mid);
        const [x2, y2] = polarToCart(half, half, ro, mid);
        return (
          <line
            key={`hr${p.id}`}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="rgba(0,0,0,0.08)"
            strokeWidth="0.5"
          />
        );
      })}
      {MOCK_PROFILES.map((p, i) => {
        const mid = sectorMidAngle(i, n);
        const labR = ri + (ro - ri) * 0.5;
        const [lx, ly] = polarToCart(half, half, labR, mid);
        const isH = i === HOVERED_IDX;
        const accent = FALLBACK_PALETTE[i % FALLBACK_PALETTE.length];
        return (
          <text
            key={`lb${p.id}`}
            x={lx}
            y={ly}
            textAnchor="middle"
            dominantBaseline="central"
            fill={isH ? accent : "rgba(28,28,32,0.65)"}
            fontWeight={isH ? 700 : 500}
            fontSize={9}
          >
            {profileMonogram(p)}
          </text>
        );
      })}
      <circle
        cx={half}
        cy={half}
        r={ri}
        fill="none"
        stroke="rgba(0,0,0,0.08)"
        strokeWidth="0.5"
      />
      <circle
        cx={half}
        cy={half}
        r={ro}
        fill="none"
        stroke="rgba(0,0,0,0.08)"
        strokeWidth="0.5"
      />
    </>
  );
}

function CrownCenter({ W, half }: { W: number; half: number }) {
  const ri = W * 0.27;
  const hovered = MOCK_PROFILES[HOVERED_IDX];
  const accent = FALLBACK_PALETTE[HOVERED_IDX % FALLBACK_PALETTE.length];
  return (
    <div
      style={{
        position: "absolute",
        left: half - ri,
        top: half - ri,
        width: 2 * ri,
        height: 2 * ri,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 2,
        textAlign: "center",
        pointerEvents: "none",
      }}
    >
      <div
        style={{
          width: 22,
          height: 22,
          borderRadius: "50%",
          background: accent,
          color: "white",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: 11,
          fontWeight: 600,
          boxShadow:
            "inset 0 0 0 0.5px rgba(0,0,0,0.12), 0 2px 6px rgba(0,0,0,0.18)",
        }}
      >
        {profileMonogram(hovered)}
      </div>
      <div
        style={{
          fontSize: 9,
          fontWeight: 600,
          color: accent,
          letterSpacing: "-0.01em",
          lineHeight: 1,
        }}
      >
        {hovered.name}
      </div>
    </div>
  );
}
