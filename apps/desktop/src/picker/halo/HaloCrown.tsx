// γ Crown — Apple-Watch digital crown. The wheel surrounds a center
// "display" that shows the currently-aimed profile (avatar + name +
// email). When no profile is aimed, the center previews the default
// profile so ⏎ is unambiguous.

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

const W = 500;

export function HaloCrownPortal({
  tileRect,
  browser,
  hoveredProfile,
  geometry,
}: PortalProps) {
  const half = W / 2;
  // Crown widens the inner radius — the center display lives inside it.
  const ri = Math.max(70, geometry.innerRadius + 30);
  const ro = geometry.outerRadius + 4;
  const n = browser.profiles.length;

  const cx = tileRect.left + tileRect.width / 2;
  const cy = tileRect.top + tileRect.height / 2;
  const anchor = {
    left: cx - half,
    top: cy - half,
    width: W,
    height: W,
  };

  const profile =
    hoveredProfile != null ? browser.profiles[hoveredProfile] : null;
  const defaultIdx = Math.max(
    0,
    browser.profiles.findIndex((p) => p.is_default),
  );
  const defaultProfile = browser.profiles[defaultIdx];
  const isIdle = profile == null;
  const showProfile = profile ?? defaultProfile;
  const showIdx = profile != null ? (hoveredProfile as number) : defaultIdx;
  const showAccent = profileAccent(showProfile, showIdx);

  return createPortal(
    <div
      className="wh-halo-portal halo-crown"
      style={{
        position: "fixed",
        pointerEvents: "auto",
        zIndex: 9999,
        ...anchor,
      }}
    >
      <div
        className="halo-frost-disc strong"
        style={{
          left: half - (ro + 4),
          top: half - (ro + 4),
          width: 2 * (ro + 4),
          height: 2 * (ro + 4),
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
          const a1 = mid - halfA + 0.01;
          const a2 = mid + halfA - 0.01;
          const isH = hoveredProfile === i;
          const restFill =
            i % 2 === 0 ? "rgba(255,255,255,0.10)" : "rgba(255,255,255,0.06)";
          return (
            <g key={p.id}>
              <path
                d={sectorPath(half, half, ri, ro, a1, a2)}
                fill={isH ? "rgba(255,255,255,0.28)" : restFill}
                style={{ transition: "fill 130ms linear" }}
              />
              {isH && (
                <path
                  d={sectorPath(half, half, ro - 5, ro, a1, a2)}
                  fill={profileAccent(p, i)}
                />
              )}
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

        {/* Letters — larger because the wheel itself is bigger */}
        {browser.profiles.map((p, i) => {
          const mid = sectorMidAngle(i, n);
          const labR = ri + (ro - ri) * 0.5;
          const [lx, ly] = polarToCart(half, half, labR, mid);
          const isH = hoveredProfile === i;
          const fs = n > 14 ? 15 : n > 10 ? 18 : 21;
          return (
            <text
              key={`lb${p.id}`}
              x={lx}
              y={ly}
              textAnchor="middle"
              dominantBaseline="central"
              fill={isH ? profileAccent(p, i) : "rgba(28,28,32,0.65)"}
              fontWeight={isH ? 700 : 500}
              fontSize={fs}
              style={{
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
          stroke="rgba(0,0,0,0.10)"
          strokeWidth="0.5"
        />

        {geometry.showNumbers && (
          <NumberBadges
            count={n}
            half={half}
            ri={ri}
            hoveredProfile={hoveredProfile}
            small
          />
        )}
      </svg>

      {/* Center display — the truth of what's selected */}
      <div
        className="halo-crown-center"
        style={{
          left: half - ri,
          top: half - ri,
          width: 2 * ri,
          height: 2 * ri,
        }}
      >
        <div className="halo-crown-browser">
          <span>{browser.name}</span>
        </div>
        <div
          className={
            "halo-crown-avatar" + (isIdle ? " idle-default" : "")
          }
          style={{ background: showAccent }}
        >
          {profileMonogram(showProfile)}
        </div>
        <div
          className={
            "halo-crown-name" + (isIdle ? " idle-default-name" : "")
          }
          style={{ color: showAccent }}
        >
          {showProfile.name}
          {isIdle && <span className="halo-crown-tag">DEFAULT</span>}
        </div>
        {showProfile.email && (
          <div className="halo-crown-email">{showProfile.email}</div>
        )}
        {isIdle && (
          <div className="halo-crown-hint">
            <span className="pk-kbd">⏎</span> open · aim to switch
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
