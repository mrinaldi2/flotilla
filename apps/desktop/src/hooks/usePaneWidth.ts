// A side pane the owner drags wider or narrower (0.22.0): the Plan tab's
// tree and its gate panel. The edge is a separator between the pane and the
// main area; the width is remembered per pane.

import type { CSSProperties, KeyboardEvent, PointerEvent } from "react";
import { useState } from "react";
import { readStored, writeStored } from "../storage";

const STEP = 40;

export interface PaneSpec {
  /** Where the width is remembered. */
  readonly key: string;
  readonly initial: number;
  readonly min: number;
  readonly max: number;
  /** The pane's side of the edge: before it (a left pane) or after it. */
  readonly pane: "before" | "after";
}

function clamp(spec: PaneSpec, width: number): number {
  return Math.round(Math.min(spec.max, Math.max(spec.min, width)));
}

function load(spec: PaneSpec): number {
  try {
    const raw = readStored(spec.key);
    const width = raw === null ? Number.NaN : Number(raw);
    return Number.isFinite(width) ? clamp(spec, width) : spec.initial;
  } catch {
    return spec.initial;
  }
}

function save(spec: PaneSpec, width: number): void {
  try {
    writeStored(spec.key, String(width));
  } catch {
    // localStorage unavailable; the pane opens at its default width
  }
}

/** The width that puts the pane's inner edge under the pointer. */
function dragged(spec: PaneSpec, event: PointerEvent<HTMLElement>): number | null {
  const edge = event.currentTarget;
  const pane = spec.pane === "before" ? edge.previousElementSibling : edge.nextElementSibling;
  if (pane === null) {
    return null;
  }
  const rect = pane.getBoundingClientRect();
  return clamp(
    spec,
    spec.pane === "before" ? event.clientX - rect.left : rect.right - event.clientX,
  );
}

/** Arrow keys move the edge the way they point. */
function keyed(spec: PaneSpec, key: string, width: number): number | null {
  const grow = spec.pane === "before" ? "ArrowRight" : "ArrowLeft";
  const shrink = spec.pane === "before" ? "ArrowLeft" : "ArrowRight";
  if (key === grow) {
    return clamp(spec, width + STEP);
  }
  return key === shrink ? clamp(spec, width - STEP) : null;
}

export interface PaneWidth {
  /** For the pane: its width. */
  readonly style: CSSProperties;
  /** For the edge element, a vertical separator beside the pane. */
  readonly edge: {
    readonly onPointerDown: (event: PointerEvent<HTMLElement>) => void;
    readonly onPointerMove: (event: PointerEvent<HTMLElement>) => void;
    readonly onPointerUp: (event: PointerEvent<HTMLElement>) => void;
    readonly onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
    readonly onDoubleClick: () => void;
    readonly "aria-valuenow": number;
    readonly "aria-valuemin": number;
    readonly "aria-valuemax": number;
  };
}

export function usePaneWidth(spec: PaneSpec): PaneWidth {
  const [width, setWidth] = useState(() => load(spec));
  const set = (next: number, keep = true): void => {
    setWidth(next);
    if (keep) {
      save(spec, next);
    }
  };
  const drag = (event: PointerEvent<HTMLElement>, keep: boolean): void => {
    const next = dragged(spec, event);
    if (next !== null) {
      set(next, keep);
    }
  };
  return {
    style: { flexBasis: `${width}px` },
    edge: {
      onPointerDown: (event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
      },
      onPointerMove: (event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          drag(event, false);
        }
      },
      onPointerUp: (event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId);
          drag(event, true);
        }
      },
      onKeyDown: (event) => {
        const next = keyed(spec, event.key, width);
        if (next !== null) {
          event.preventDefault();
          set(next);
        }
      },
      onDoubleClick: () => set(spec.initial),
      "aria-valuenow": width,
      "aria-valuemin": spec.min,
      "aria-valuemax": spec.max,
    },
  };
}
