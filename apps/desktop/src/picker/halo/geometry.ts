// Polar/Cartesian helpers for the Halo wheel. Pure math — no React, no DOM —
// so every variant can rely on the exact same hit-test and sector painter.

export function polarToCart(
  cx: number,
  cy: number,
  r: number,
  a: number,
): [number, number] {
  return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
}

/** SVG path `d` for an annular sector from angle `a1` to `a2`, inner radius
 *  `ri`, outer radius `ro`. Used by Frosted (full fill), Bezel (hover-only
 *  fill), and Crown (subtle tonal fill). */
export function sectorPath(
  cx: number,
  cy: number,
  ri: number,
  ro: number,
  a1: number,
  a2: number,
): string {
  const [x1, y1] = polarToCart(cx, cy, ro, a1);
  const [x2, y2] = polarToCart(cx, cy, ro, a2);
  const [x3, y3] = polarToCart(cx, cy, ri, a2);
  const [x4, y4] = polarToCart(cx, cy, ri, a1);
  const large = a2 - a1 > Math.PI ? 1 : 0;
  return `M ${x1} ${y1} A ${ro} ${ro} 0 ${large} 1 ${x2} ${y2} L ${x3} ${y3} A ${ri} ${ri} 0 ${large} 0 ${x4} ${y4} Z`;
}

/** Map a screen-space (mouse) point to the sector index it falls in, or
 *  `null` when it's outside the annulus.
 *
 *  The wheel arranges N sectors starting at -π/2 (12 o'clock) and going
 *  clockwise. Sector i spans the wedge centered at -π/2 + 2π·i/N. */
export function hitTestSector(
  mouseX: number,
  mouseY: number,
  cx: number,
  cy: number,
  innerR: number,
  outerR: number,
  n: number,
): number | null {
  const dx = mouseX - cx;
  const dy = mouseY - cy;
  const dist = Math.hypot(dx, dy);
  if (dist < innerR || dist > outerR) return null;
  const ang = Math.atan2(dy, dx);
  const baseline = -Math.PI / 2 - Math.PI / n;
  let t = (ang - baseline) / (Math.PI * 2);
  t = ((t % 1) + 1) % 1;
  return Math.floor(t * n);
}

/** Position helper for sector midpoints / label points. */
export function sectorMidAngle(i: number, n: number): number {
  return -Math.PI / 2 + (2 * Math.PI * i) / n;
}

/** Position helper for sector boundary lines. */
export function sectorBoundaryAngle(i: number, n: number): number {
  return sectorMidAngle(i, n) - Math.PI / n;
}
