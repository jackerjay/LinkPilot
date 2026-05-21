// α Frosted — translucent white sectors, profile color as a thin band on
// the outer rim. Closest to macOS popover language (frosted glass + 0.5px
// hairlines + colored accents only on hover).

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

const W = 460;

export function HaloFrostedPortal({
  tileRect,
  browser,
  hoveredProfile,
  geometry,
}: PortalProps) {
  const half = W / 2;
  const ri = geometry.innerRadius;
  const ro = geometry.outerRadius;
  const rimRest = Math.max(3, geometry.rimWidth - 1);
  const rimHover = geometry.rimWidth + 5;
  const hoverPush = geometry.hoverPush;
  const n = browser.profiles.length;

  // Anchor the wheel on top of the armed tile.
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
      className="wh-halo-portal halo-frosted"
      style={{
        position: "fixed",
        pointerEvents: "auto",
        zIndex: 9999,
        ...anchor,
      }}
    >
      <div
        className="halo-frost-disc"
        style={{
          left: half - (ro + 6),
          top: half - (ro + 6),
          width: 2 * (ro + 6),
          height: 2 * (ro + 6),
        }}
      />

      <svg
        width={W}
        height={W}
        viewBox={`0 0 ${W} ${W}`}
        style={{ position: "absolute", inset: 0, overflow: "visible" }}
      >
        {browser.profiles.map((p, i) => {
          const halfA = Math.PI / n;
          const mid = sectorMidAngle(i, n);
          const a1 = mid - halfA + 0.012;
          const a2 = mid + halfA - 0.012;
          const isH = hoveredProfile === i;
          const ro_ = ro + (isH ? hoverPush : 0);
          const rimW = isH ? rimHover : rimRest;
          const accent = profileAccent(p, i);
          return (
            <g key={p.id}>
              <path
                d={sectorPath(half, half, ri, ro_, a1, a2)}
                fill={
                  isH ? "rgba(255,255,255,0.24)" : "rgba(255,255,255,0.04)"
                }
                style={{ transition: "fill 130ms linear" }}
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
        {browser.profiles.map((p, i) => {
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

        {/* Letter monograms (mono font) */}
        {browser.profiles.map((p, i) => {
          const mid = sectorMidAngle(i, n);
          const labR = ri + (ro - ri) * 0.48;
          const [lx, ly] = polarToCart(half, half, labR, mid);
          const isH = hoveredProfile === i;
          const fs = n > 14 ? 12 : n > 10 ? 13 : 15;
          const accent = profileAccent(p, i);
          return (
            <text
              key={`lb${p.id}`}
              x={lx}
              y={ly}
              textAnchor="middle"
              dominantBaseline="central"
              fill={isH ? accent : "rgba(28,28,32,0.7)"}
              fontWeight={isH ? 600 : 500}
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

        {/* Annulus boundary hairlines */}
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
