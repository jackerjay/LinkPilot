// Inline Halo renderer for Settings. The style picker uses it as a static
// preview; the profile-order popup reuses the same paint path with real
// profiles so configuration matches the selected Halo style.

import {
  useCallback,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
  type SVGProps,
} from "react";
import type { PickerStyle } from "@/lib/types";
import {
  hitTestSector,
  polarToCart,
  sectorBoundaryAngle,
  sectorMidAngle,
  sectorPath,
} from "./geometry";
import {
  profileAccent,
  profileMonogram,
  type PickerProfile,
} from "./types";

const MOCK_PROFILES: PickerProfile[] = [
  { id: "default", name: "Default", email: "you@gmail.com", is_default: true },
  { id: "work", name: "Work", email: "you@company.example", is_default: false },
  { id: "side", name: "Side", email: "side@me.com", is_default: false },
  { id: "oss", name: "OSS", email: "you@oss.dev", is_default: false },
  { id: "personal", name: "Personal", email: "you@me.com", is_default: false },
];

const HOVERED_IDX = 1;

interface HaloPreviewProps {
  style: PickerStyle;
  /** Render width in CSS pixels. The viewport is square. Default 160. */
  size?: number;
  profiles?: PickerProfile[];
  /** Highlighted profile slot. Defaults to the deterministic Settings preview. */
  activeIndex?: number;
  /** Persistent selected slot used by the profile-order popup. */
  selectedIndex?: number;
  interactive?: boolean;
  addSlot?: boolean;
  draggable?: boolean;
  onSelectIndex?: (index: number) => void;
  onReorder?: (fromIndex: number, toIndex: number) => void;
}

interface SectorProps {
  W: number;
  slots: HaloSlot[];
  activeIndex: number;
  selectedIndex?: number;
  draggingSlotId?: string | null;
  dropIndex?: number | null;
  interactive: boolean;
  onSelectIndex?: (index: number) => void;
}

type HaloSlot =
  | { kind: "profile"; profile: PickerProfile }
  | { kind: "add"; id: "__add_profile__" };

interface DragRef {
  pointerId: number;
  fromIndex: number;
  startX: number;
  startY: number;
  moved: boolean;
  targetIndex: number | null;
  pointerX: number;
  pointerY: number;
}

interface DragState {
  fromIndex: number;
  targetIndex: number | null;
  moved: boolean;
  pointerX: number;
  pointerY: number;
}

type HaloSlotInteractionProps<T extends SVGElement> = SVGProps<T> & {
  "data-halo-slot-index"?: string;
};

export function HaloPreview({
  style,
  size = 160,
  profiles,
  activeIndex,
  selectedIndex,
  interactive = false,
  addSlot = false,
  draggable = false,
  onSelectIndex,
  onReorder,
}: HaloPreviewProps) {
  const visibleProfiles =
    profiles && profiles.length > 0 ? profiles : MOCK_PROFILES;
  const slots: HaloSlot[] = [
    ...visibleProfiles.map((profile) => ({ kind: "profile" as const, profile })),
    ...(addSlot ? [{ kind: "add" as const, id: "__add_profile__" as const }] : []),
  ];
  const fallbackIndex = profiles && profiles.length > 0 ? 0 : HOVERED_IDX;
  const active = clampIndex(
    activeIndex ?? selectedIndex ?? fallbackIndex,
    slots.length,
  );
  const [dragState, setDragState] = useState<DragState | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const dragRef = useRef<DragRef | null>(null);
  const W = size;
  const half = W / 2;
  const dragSlot =
    dragState && slots[dragState.fromIndex]?.kind === "profile"
      ? slots[dragState.fromIndex]
      : null;
  const dragSlotId = dragSlot ? slotId(dragSlot) : null;
  const visualSlots = useMemo(
    () =>
      dragState?.moved && dragSlotId
        ? reorderVisualSlots(slots, dragState.fromIndex, dragState.targetIndex)
        : slots,
    [
      dragSlotId,
      dragState?.fromIndex,
      dragState?.moved,
      dragState?.targetIndex,
      slots,
    ],
  );
  const selectedSlotId =
    selectedIndex == null ? null : slotId(slots[selectedIndex] ?? slots[0]);
  const activeSlotId = slotId(slots[active] ?? slots[0]);
  const selectedVisualIndex =
    selectedSlotId == null
      ? undefined
      : visualSlots.findIndex((slot) => slotId(slot) === selectedSlotId);
  const activeVisualIndex = Math.max(
    0,
    visualSlots.findIndex((slot) => slotId(slot) === activeSlotId),
  );
  const dragVisualIndex =
    dragSlotId == null
      ? null
      : visualSlots.findIndex((slot) => slotId(slot) === dragSlotId);
  const displayActiveIndex =
    dragState?.moved && dragVisualIndex != null && dragVisualIndex >= 0
      ? dragVisualIndex
      : activeVisualIndex;
  const dropIndex = dragState?.moved ? dragVisualIndex : null;
  const activeSlot = visualSlots[displayActiveIndex];

  const hitTestClientPoint = useCallback(
    (clientX: number, clientY: number): number | null => {
      const svg = svgRef.current;
      if (!svg || slots.length === 0) return null;
      const rect = svg.getBoundingClientRect();
      const radii = interactionRadii(style, W);
      return hitTestSector(
        clientX - rect.left,
        clientY - rect.top,
        W / 2,
        W / 2,
        radii.inner,
        radii.outer,
        slots.length,
      );
    },
    [W, slots.length, style],
  );

  const handlePointerDown = useCallback(
    (event: PointerEvent<SVGSVGElement>) => {
      if (!interactive) return;
      const target = event.target;
      if (!(target instanceof Element)) return;
      const slotNode = target.closest("[data-halo-slot-index]");
      const rawIndex = slotNode?.getAttribute("data-halo-slot-index");
      if (!rawIndex) return;
      const index = Number(rawIndex);
      if (!Number.isInteger(index) || index < 0 || index >= slots.length) {
        return;
      }

      event.preventDefault();
      if (slots[index]?.kind === "add") {
        onSelectIndex?.(index);
        return;
      }

      if (!draggable) {
        onSelectIndex?.(index);
        return;
      }

      event.currentTarget.setPointerCapture(event.pointerId);
      const pointer = localPoint(
        event.currentTarget,
        event.clientX,
        event.clientY,
      );
      dragRef.current = {
        pointerId: event.pointerId,
        fromIndex: index,
        startX: event.clientX,
        startY: event.clientY,
        moved: false,
        targetIndex: index,
        pointerX: event.clientX,
        pointerY: event.clientY,
      };
      setDragState({
        fromIndex: index,
        targetIndex: index,
        moved: false,
        pointerX: pointer.x,
        pointerY: pointer.y,
      });
      onSelectIndex?.(index);
    },
    [draggable, interactive, onSelectIndex, slots],
  );

  const handlePointerMove = useCallback(
    (event: PointerEvent<SVGSVGElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;

      const moved =
        drag.moved ||
        Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) > 4;
      if (!moved) return;

      const targetIndex = hitTestClientPoint(event.clientX, event.clientY);
      const pointer = localPoint(
        event.currentTarget,
        event.clientX,
        event.clientY,
      );
      dragRef.current = {
        ...drag,
        moved: true,
        targetIndex,
        pointerX: event.clientX,
        pointerY: event.clientY,
      };
      setDragState({
        fromIndex: drag.fromIndex,
        targetIndex,
        moved: true,
        pointerX: pointer.x,
        pointerY: pointer.y,
      });
    },
    [hitTestClientPoint],
  );

  const finishPointerDrag = useCallback(
    (event: PointerEvent<SVGSVGElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }

      const targetIndex =
        drag.moved && drag.targetIndex == null
          ? hitTestClientPoint(event.clientX, event.clientY)
          : drag.targetIndex;
      dragRef.current = null;
      setDragState(null);

      if (!drag.moved) {
        onSelectIndex?.(drag.fromIndex);
        return;
      }
      if (targetIndex == null || targetIndex === drag.fromIndex) return;
      const profileCount = visibleProfiles.length;
      const toIndex = Math.min(targetIndex, profileCount - 1);
      if (toIndex !== drag.fromIndex) onReorder?.(drag.fromIndex, toIndex);
    },
    [hitTestClientPoint, onReorder, onSelectIndex, visibleProfiles.length],
  );

  const cancelPointerDrag = useCallback((event: PointerEvent<SVGSVGElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    dragRef.current = null;
    setDragState(null);
  }, []);

  return (
    <div
      className={`halo-preview-inline${interactive ? " interactive" : ""}${
        dragState?.moved ? " dragging" : ""
      }`}
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
        ref={svgRef}
        width={W}
        height={W}
        viewBox={`0 0 ${W} ${W}`}
        style={{
          position: "absolute",
          inset: 0,
          overflow: "visible",
          touchAction: "none",
        }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishPointerDrag}
        onPointerCancel={cancelPointerDrag}
      >
        {style === "frosted" && (
          <FrostedSectors
            W={W}
            slots={visualSlots}
            activeIndex={displayActiveIndex}
            selectedIndex={normalizeVisualIndex(selectedVisualIndex)}
            draggingSlotId={dragState?.moved ? dragSlotId : null}
            dropIndex={dropIndex}
            interactive={interactive}
            onSelectIndex={onSelectIndex}
          />
        )}
        {style === "bezel" && (
          <BezelSectors
            W={W}
            slots={visualSlots}
            activeIndex={displayActiveIndex}
            selectedIndex={normalizeVisualIndex(selectedVisualIndex)}
            draggingSlotId={dragState?.moved ? dragSlotId : null}
            dropIndex={dropIndex}
            interactive={interactive}
            onSelectIndex={onSelectIndex}
          />
        )}
        {style === "crown" && (
          <CrownSectors
            W={W}
            slots={visualSlots}
            activeIndex={displayActiveIndex}
            selectedIndex={normalizeVisualIndex(selectedVisualIndex)}
            draggingSlotId={dragState?.moved ? dragSlotId : null}
            dropIndex={dropIndex}
            interactive={interactive}
            onSelectIndex={onSelectIndex}
          />
        )}
      </svg>

      {style === "crown" && activeSlot?.kind === "profile" && (
        <CrownCenter
          W={W}
          half={half}
          profile={activeSlot.profile}
          accent={slotAccent(activeSlot, displayActiveIndex)}
        />
      )}

      {dragState?.moved && dragSlot?.kind === "profile" && (
        <div
          className="halo-drag-ghost"
          style={{
            left: dragState.pointerX,
            top: dragState.pointerY,
            background: profileAccent(dragSlot.profile, dragState.fromIndex),
          }}
        >
          {profileMonogram(dragSlot.profile)}
        </div>
      )}
    </div>
  );
}

function FrostedSectors({
  W,
  slots,
  activeIndex,
  selectedIndex,
  draggingSlotId,
  dropIndex,
  interactive,
  onSelectIndex,
}: SectorProps) {
  const half = W / 2;
  const ri = W * 0.18;
  const ro = W * 0.42;
  const rimRest = 2;
  const rimHover = 4;
  const hoverPush = 3;
  const n = slots.length;
  return (
    <>
      {slots.map((slot, i) => {
        const halfA = Math.PI / n;
        const mid = sectorMidAngle(i, n);
        const a1 = mid - halfA + 0.012;
        const a2 = mid + halfA - 0.012;
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isDragging = slotId(slot) === draggingSlotId;
        const isDrop = i === dropIndex;
        const isAdd = slot.kind === "add";
        const ro_ = ro + (isActive ? hoverPush : 0);
        const rimW = isActive ? rimHover : rimRest;
        const accent = slotAccent(slot, i);
        return (
          <g
            key={slotId(slot)}
            {...sectorInteraction<SVGGElement>(
              slot,
              i,
              interactive,
              onSelectIndex,
            )}
          >
            <path
              d={sectorPath(half, half, ri, ro_, a1, a2)}
              fill={
                isAdd
                  ? "rgba(255,255,255,0.10)"
                  : isActive || isSelected || isDrop
                  ? "rgba(255,255,255,0.24)"
                  : "rgba(255,255,255,0.04)"
              }
              opacity={isDragging ? 0.34 : 1}
            />
            <path
              d={sectorPath(half, half, ro_ - rimW, ro_, a1, a2)}
              fill={accent}
              opacity={isAdd ? 0 : isActive || isSelected ? 0.78 : 0.42}
            />
            {(isSelected || isDrop) && !isAdd && (
              <path
                d={sectorPath(half, half, ri, ro_, a1, a2)}
                fill="none"
                stroke={accent}
                strokeWidth="1.2"
                opacity="0.64"
              />
            )}
          </g>
        );
      })}
      {slots.map((slot, i) => {
        const mid = sectorBoundaryAngle(i, n);
        const [x1, y1] = polarToCart(half, half, ri, mid);
        const [x2, y2] = polarToCart(half, half, ro, mid);
        return (
          <line
            key={`hr${slotId(slot)}`}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
            strokeWidth="0.5"
          />
        );
      })}
      {slots.map((slot, i) => {
        const mid = sectorMidAngle(i, n);
        const labR = ri + (ro - ri) * 0.48;
        const [lx, ly] = polarToCart(half, half, labR, mid);
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isAdd = slot.kind === "add";
        const accent = slotAccent(slot, i);
        return (
          <text
            key={`lb${slotId(slot)}`}
            x={lx}
            y={ly}
            textAnchor="middle"
            dominantBaseline="central"
            fill={
              isActive || isSelected
                ? accent
                : "var(--halo-preview-label, rgba(28,28,32,0.7))"
            }
            fontWeight={isActive || isSelected ? 600 : 500}
            fontSize={isAdd ? 18 : 9}
            style={{
              fontFamily:
                isAdd
                  ? "-apple-system, BlinkMacSystemFont, sans-serif"
                  : 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
              pointerEvents: "none",
            }}
          >
            {slotMonogram(slot)}
          </text>
        );
      })}
      <circle
        cx={half}
        cy={half}
        r={ri}
        fill="none"
        stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
        strokeWidth="0.5"
      />
      <circle
        cx={half}
        cy={half}
        r={ro}
        fill="none"
        stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
        strokeWidth="0.5"
      />
    </>
  );
}

function BezelSectors({
  W,
  slots,
  activeIndex,
  selectedIndex,
  draggingSlotId,
  dropIndex,
  interactive,
  onSelectIndex,
}: SectorProps) {
  const half = W / 2;
  const ri = W * 0.18;
  const ro = W * 0.42;
  const dotR = ri + (ro - ri) * 0.55;
  const labelR = ro + 8;
  const n = slots.length;
  return (
    <>
      {slots.map((slot, i) => {
        const halfA = Math.PI / n;
        const mid = sectorMidAngle(i, n);
        const a1 = mid - halfA + 0.006;
        const a2 = mid + halfA - 0.006;
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isDragging = slotId(slot) === draggingSlotId;
        const isDrop = i === dropIndex;
        const isAdd = slot.kind === "add";
        return (
          <path
            key={`wg${slotId(slot)}`}
            d={sectorPath(half, half, ri, ro, a1, a2)}
            fill={slotAccent(slot, i)}
            opacity={
              isDragging ? 0.08 : isAdd ? 0.06 : isActive || isSelected || isDrop ? 0.18 : 0
            }
            {...sectorInteraction<SVGPathElement>(
              slot,
              i,
              interactive,
              onSelectIndex,
            )}
          />
        );
      })}
      <circle
        cx={half}
        cy={half}
        r={ro}
        fill="none"
        stroke="var(--halo-preview-line-strong, rgba(0,0,0,0.14))"
        strokeWidth="1"
      />
      <circle
        cx={half}
        cy={half}
        r={ri}
        fill="none"
        stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
        strokeWidth="0.5"
      />
      {slots.map((slot, i) => {
        const mid = sectorBoundaryAngle(i, n);
        const [x1, y1] = polarToCart(half, half, ro - 5, mid);
        const [x2, y2] = polarToCart(half, half, ro, mid);
        return (
          <line
            key={`tk${slotId(slot)}`}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="var(--halo-preview-tick, rgba(0,0,0,0.30))"
            strokeWidth="1"
          />
        );
      })}
      {slots.map((slot, i) => {
        const mid = sectorMidAngle(i, n);
        const [dx, dy] = polarToCart(half, half, dotR, mid);
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isAdd = slot.kind === "add";
        return (
          <circle
            key={`dt${slotId(slot)}`}
            cx={dx}
            cy={dy}
            r={isAdd ? 8 : isActive || isSelected ? 6 : 4}
            fill={isAdd ? "rgba(255,255,255,0.42)" : slotAccent(slot, i)}
            opacity={isActive || isSelected || isAdd ? 1 : 0.78}
            stroke={isSelected ? "rgba(255,255,255,0.78)" : "transparent"}
            strokeWidth={isAdd ? "1" : "1.2"}
            {...sectorInteraction<SVGCircleElement>(
              slot,
              i,
              interactive,
              onSelectIndex,
            )}
          />
        );
      })}
      {slots.map((slot, i) => {
        const mid = sectorMidAngle(i, n);
        const [lx, ly] = polarToCart(half, half, labelR, mid);
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isAdd = slot.kind === "add";
        const accent = slotAccent(slot, i);
        return (
          <text
            key={`lb${slotId(slot)}`}
            x={lx}
            y={ly}
            textAnchor="middle"
            dominantBaseline="central"
            fill={
              isActive || isSelected
                ? accent
                : "var(--halo-preview-label, rgba(28,28,32,0.78))"
            }
            fontWeight={isActive || isSelected ? 700 : 500}
            fontSize={isAdd ? 17 : 8}
            style={{
              fontFamily:
                isAdd
                  ? "-apple-system, BlinkMacSystemFont, sans-serif"
                  : 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
              pointerEvents: "none",
            }}
          >
            {slotMonogram(slot)}
          </text>
        );
      })}
    </>
  );
}

function CrownSectors({
  W,
  slots,
  activeIndex,
  selectedIndex,
  draggingSlotId,
  dropIndex,
  interactive,
  onSelectIndex,
}: SectorProps) {
  const half = W / 2;
  const ri = W * 0.27;
  const ro = W * 0.44;
  const n = slots.length;
  return (
    <>
      {slots.map((slot, i) => {
        const halfA = Math.PI / n;
        const mid = sectorMidAngle(i, n);
        const a1 = mid - halfA + 0.01;
        const a2 = mid + halfA - 0.01;
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isDragging = slotId(slot) === draggingSlotId;
        const isDrop = i === dropIndex;
        const isAdd = slot.kind === "add";
        const restFill =
          i % 2 === 0 ? "rgba(255,255,255,0.10)" : "rgba(255,255,255,0.06)";
        return (
          <g
            key={slotId(slot)}
            {...sectorInteraction<SVGGElement>(
              slot,
              i,
              interactive,
              onSelectIndex,
            )}
          >
            <path
              d={sectorPath(half, half, ri, ro, a1, a2)}
              fill={
                isAdd
                  ? "rgba(255,255,255,0.12)"
                  : isActive || isSelected || isDrop
                  ? "rgba(255,255,255,0.28)"
                  : restFill
              }
              opacity={isDragging ? 0.34 : 1}
            />
            {(isActive || isSelected || isAdd) && (
              <path
                d={sectorPath(half, half, ro - 3, ro, a1, a2)}
                fill={slotAccent(slot, i)}
                opacity={isAdd ? 0.34 : 1}
              />
            )}
          </g>
        );
      })}
      {slots.map((slot, i) => {
        const mid = sectorBoundaryAngle(i, n);
        const [x1, y1] = polarToCart(half, half, ri, mid);
        const [x2, y2] = polarToCart(half, half, ro, mid);
        return (
          <line
            key={`hr${slotId(slot)}`}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
            strokeWidth="0.5"
          />
        );
      })}
      {slots.map((slot, i) => {
        const mid = sectorMidAngle(i, n);
        const labR = ri + (ro - ri) * 0.5;
        const [lx, ly] = polarToCart(half, half, labR, mid);
        const isActive = i === activeIndex;
        const isSelected = i === selectedIndex;
        const isAdd = slot.kind === "add";
        const accent = slotAccent(slot, i);
        return (
          <text
            key={`lb${slotId(slot)}`}
            x={lx}
            y={ly}
            textAnchor="middle"
            dominantBaseline="central"
            fill={
              isActive || isSelected
                ? accent
                : "var(--halo-preview-label-muted, rgba(28,28,32,0.65))"
            }
            fontWeight={isActive || isSelected ? 700 : 500}
            fontSize={isAdd ? 18 : 9}
            style={{
              pointerEvents: "none",
              fontFamily: isAdd
                ? "-apple-system, BlinkMacSystemFont, sans-serif"
                : undefined,
            }}
          >
            {slotMonogram(slot)}
          </text>
        );
      })}
      <circle
        cx={half}
        cy={half}
        r={ri}
        fill="none"
        stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
        strokeWidth="0.5"
      />
      <circle
        cx={half}
        cy={half}
        r={ro}
        fill="none"
        stroke="var(--halo-preview-line, rgba(0,0,0,0.08))"
        strokeWidth="0.5"
      />
    </>
  );
}

function CrownCenter({
  W,
  half,
  profile,
  accent,
}: {
  W: number;
  half: number;
  profile: PickerProfile;
  accent: string;
}) {
  const ri = W * 0.27;
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
        {profileMonogram(profile)}
      </div>
      <div
        style={{
          fontSize: 9,
          fontWeight: 600,
          color: accent,
          lineHeight: 1,
        }}
      >
        {profile.name}
      </div>
    </div>
  );
}

function sectorInteraction<T extends SVGElement>(
  slot: HaloSlot,
  index: number,
  interactive: boolean,
  onSelectIndex?: (index: number) => void,
): HaloSlotInteractionProps<T> {
  if (!interactive) return {};
  return {
    role: "button",
    tabIndex: 0,
    "aria-label":
      slot.kind === "add" ? "Add profile" : `Select ${slot.profile.name}`,
    "data-halo-slot-index": String(index),
    style: { cursor: "pointer" },
    onKeyDown: (event: KeyboardEvent<T>) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        onSelectIndex?.(index);
      }
    },
  };
}

function clampIndex(index: number, length: number): number {
  return Math.min(Math.max(index, 0), Math.max(length - 1, 0));
}

function slotId(slot: HaloSlot): string {
  return slot.kind === "add" ? slot.id : slot.profile.id;
}

function slotAccent(slot: HaloSlot, idx: number): string {
  return slot.kind === "add"
    ? "var(--mac-accent)"
    : profileAccent(slot.profile, idx);
}

function slotMonogram(slot: HaloSlot): string {
  return slot.kind === "add" ? "+" : profileMonogram(slot.profile);
}

function normalizeVisualIndex(index: number | undefined): number | undefined {
  return index == null || index < 0 ? undefined : index;
}

function reorderVisualSlots(
  slots: HaloSlot[],
  fromIndex: number,
  targetIndex: number | null,
): HaloSlot[] {
  const profileSlots = slots.filter((slot) => slot.kind === "profile");
  const addSlot = slots.find((slot) => slot.kind === "add") ?? null;
  if (fromIndex < 0 || fromIndex >= profileSlots.length) return slots;
  if (targetIndex == null) return slots;

  const toIndex = Math.min(Math.max(targetIndex, 0), profileSlots.length - 1);
  const next = [...profileSlots];
  const [moved] = next.splice(fromIndex, 1);
  if (!moved) return slots;
  next.splice(toIndex, 0, moved);
  return addSlot ? [...next, addSlot] : next;
}

function localPoint(
  node: Element,
  clientX: number,
  clientY: number,
): { x: number; y: number } {
  const rect = node.getBoundingClientRect();
  return { x: clientX - rect.left, y: clientY - rect.top };
}

function interactionRadii(
  style: PickerStyle,
  W: number,
): { inner: number; outer: number } {
  return style === "crown"
    ? { inner: W * 0.27, outer: W * 0.44 }
    : { inner: W * 0.18, outer: W * 0.42 };
}
