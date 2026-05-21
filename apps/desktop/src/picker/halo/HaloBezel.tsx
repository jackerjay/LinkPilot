// β Bezel — instrument-ring look. At rest the sectors are invisible:
// only the outer ring + tick marks + colored profile dots. Hover paints
// the wedge in the profile color.

import { createPortal } from "react-dom";
import {
  polarToCart,
  sectorBoundaryAngle,
  sectorMidAngle,
  sectorPath,
} from "./geometry";
import { NumberBadges } from "./NumberBadges";
import {
  type HaloGeometry,
  type PickerChoice,
  profileAccent,
  profileMonogram,
} from "./types";

interface PortalProps {
  tileRect: DOMRect;
  browser: PickerChoice;
  hoveredProfile: number | null;
  geometry: HaloGeometry;
}

const W = 480;

export function HaloBezelPortal({
  tileRect,
  browser,
  hoveredProfile,
  geometry,
}: PortalProps) {
  const half = W / 2;
  const ri = geometry.innerRadius + 2;
  const ro = geometry.outerRadius;
  const dotR = ri + (ro - ri) * 0.55;
  const labelR = ro + 14;
  const n = browser.profiles.length;

  const cx = tileRect.left + tileRect.width / 2;
  const cy = tileRect.top + tileRect.height / 2;
  const anchor = {
    left: cx - half,
    top: cy - half,
    width: W,
    height: W,
  };

  return createPortal(
    <div
      className="wh-halo-portal halo-bezel"
      style={{
        position: "fixed",
        pointerEvents: "auto",
        zIndex: 9999,
        ...anchor,
      }}
    >
      <div
        className="halo-frost-disc subtle"
        style={{
          left: half - (ro + 2),
          top: half - (ro + 2),
          width: 2 * (ro + 2),
          height: 2 * (ro + 2),
        }}
      />

      <svg
        width={W}
        height={W}
        viewBox={`0 0 ${W} ${W}`}
        style={{ position: "absolute", inset: 0, overflow: "visible" }}
      >
        {/* Hovered wedge fill — the only colored sector at any time */}
        {hoveredProfile != null &&
          (() => {
            const i = hoveredProfile;
            const halfA = Math.PI / n;
            const mid = sectorMidAngle(i, n);
            const a1 = mid - halfA + 0.006;
            const a2 = mid + halfA - 0.006;
            return (
              <path
                d={sectorPath(half, half, ri, ro, a1, a2)}
                fill={profileAccent(browser.profiles[i], i)}
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

        {/* Tick marks at sector boundaries */}
        {browser.profiles.map((_, i) => {
          const mid = sectorBoundaryAngle(i, n);
          const [x1, y1] = polarToCart(half, half, ro - 8, mid);
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

        {/* Profile dots — color always visible */}
        {browser.profiles.map((p, i) => {
          const mid = sectorMidAngle(i, n);
          const [dx, dy] = polarToCart(half, half, dotR, mid);
          const isH = hoveredProfile === i;
          return (
            <circle
              key={`dt${p.id}`}
              cx={dx}
              cy={dy}
              r={isH ? 9 : 6}
              fill={profileAccent(p, i)}
              opacity={isH ? 1 : 0.78}
              style={{
                transition:
                  "r 130ms cubic-bezier(.2,.7,.3,1), opacity 130ms linear",
              }}
            />
          );
        })}

        {/* Letter monograms outside the ring (etched look) */}
        {browser.profiles.map((p, i) => {
          const mid = sectorMidAngle(i, n);
          const [lx, ly] = polarToCart(half, half, labelR, mid);
          const isH = hoveredProfile === i;
          const fs = n > 14 ? 11 : n > 10 ? 12 : 13;
          return (
            <text
              key={`lb${p.id}`}
              x={lx}
              y={ly}
              textAnchor="middle"
              dominantBaseline="central"
              fill={isH ? profileAccent(p, i) : "rgba(28,28,32,0.78)"}
              fontWeight={isH ? 700 : 500}
              fontSize={fs}
              style={{
                fontFamily:
                  'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
                pointerEvents: "none",
                transition: "fill 120ms linear",
              }}
            >
              {profileMonogram(p)}
            </text>
          );
        })}

        {geometry.showNumbers && (
          <NumberBadges
            count={n}
            half={half}
            ri={ri}
            hoveredProfile={hoveredProfile}
          />
        )}
      </svg>
    </div>,
    document.body,
  );
}
