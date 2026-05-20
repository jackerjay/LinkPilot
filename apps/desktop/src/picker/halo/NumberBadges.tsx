// 1-9 badges painted at the inner edge of each sector. Profiles past index
// 8 (the 10th, 11th, …) are reachable only by aim — the design declined to
// map them to 0/-/= because those keys are unmappable across keyboard
// layouts.

import { polarToCart, sectorMidAngle } from "./geometry";

interface NumberBadgesProps {
  count: number;
  half: number;
  ri: number;
  hoveredProfile: number | null;
  /** Crown uses slightly smaller badges so they don't crowd the center
   *  display. */
  small?: boolean;
}

export function NumberBadges({
  count,
  half,
  ri,
  hoveredProfile,
  small = false,
}: NumberBadgesProps) {
  const r = ri + (small ? 10 : 11);
  const dotR = small ? 7 : 8.5;
  const fs = small ? 8.5 : 9.5;
  return (
    <g style={{ pointerEvents: "none" }}>
      {Array.from({ length: Math.min(count, 9) }, (_, i) => {
        const mid = sectorMidAngle(i, count);
        const [bx, by] = polarToCart(half, half, r, mid);
        const isH = hoveredProfile === i;
        return (
          <g
            key={`num${i}`}
            style={{
              opacity: isH ? 0 : 1,
              transition: "opacity 100ms linear",
            }}
          >
            <circle
              cx={bx}
              cy={by}
              r={dotR}
              fill="rgba(255,255,255,0.96)"
              stroke="rgba(0,0,0,0.14)"
              strokeWidth="0.5"
              style={{ filter: "drop-shadow(0 1px 2px rgba(0,0,0,0.10))" }}
            />
            <text
              x={bx}
              y={by}
              textAnchor="middle"
              dominantBaseline="central"
              fontSize={fs}
              fontWeight="600"
              fill="rgba(28,28,32,0.78)"
              style={{
                fontFamily:
                  'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
              }}
            >
              {i + 1}
            </text>
          </g>
        );
      })}
    </g>
  );
}
