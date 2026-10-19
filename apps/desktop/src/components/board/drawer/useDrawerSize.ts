// The drawer's width (0.22.0): dragged from its left edge, or widened over
// the whole board, and remembered for every drawer.

import type { CSSProperties, KeyboardEvent, PointerEvent } from "react";
import { useState } from "react";
import { readStored, writeStored } from "../../../storage";

export const DRAWER_MIN = 360;
export const DRAWER_DEFAULT = 440;
const DRAWER_MAX = 2400;
const STEP = 40;
const KEY = "drawer.size";

export interface DrawerSize {
  readonly width: number;
  /** Over the whole board, whatever the width. */
  readonly wide: boolean;
}

const DEFAULT_SIZE: DrawerSize = { width: DRAWER_DEFAULT, wide: false };

function clampWidth(width: number): number {
  return Math.round(Math.min(DRAWER_MAX, Math.max(DRAWER_MIN, width)));
}

export function loadDrawerSize(): DrawerSize {
  try {
    const raw = readStored(KEY);
    const parsed: unknown = raw === null ? null : JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return DEFAULT_SIZE;
    }
    const width: unknown = Reflect.get(parsed, "width");
    return {
      width:
        typeof width === "number" && Number.isFinite(width) ? clampWidth(width) : DRAWER_DEFAULT,
      wide: Reflect.get(parsed, "wide") === true,
    };
  } catch {
    return DEFAULT_SIZE;
  }
}

function saveDrawerSize(size: DrawerSize): void {
  try {
    writeStored(KEY, JSON.stringify(size));
  } catch {
    // localStorage unavailable; the drawer opens at its default width
  }
}

/** Arrow keys on the edge: left widens, right narrows. */
function keyedWidth(key: string, width: number): number | null {
  if (key === "ArrowLeft") {
    return clampWidth(width + STEP);
  }
  if (key === "ArrowRight") {
    return clampWidth(width - STEP);
  }
  return null;
}

/** The width that puts the drawer's left edge under the pointer. */
function draggedWidth(event: PointerEvent<HTMLElement>): number | null {
  const host = event.currentTarget.parentElement?.parentElement;
  if (!host) {
    return null;
  }
  return clampWidth(host.getBoundingClientRect().right - event.clientX);
}

export interface DrawerSizing {
  readonly wide: boolean;
  readonly style: CSSProperties;
  readonly onToggleWide: () => void;
  /** Props for the drawer's left edge, a vertical separator. */
  readonly edge: {
    readonly onPointerDown: (event: PointerEvent<HTMLElement>) => void;
    readonly onPointerMove: (event: PointerEvent<HTMLElement>) => void;
    readonly onPointerUp: (event: PointerEvent<HTMLElement>) => void;
    readonly onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
    readonly onDoubleClick: () => void;
    readonly "aria-valuenow": number;
  };
}

/** `initial` pins the size for stories; otherwise the owner's last one. */
export function useDrawerSize(initial?: DrawerSize): DrawerSizing {
  const [size, setSize] = useState(() => initial ?? loadDrawerSize());
  const update = (next: DrawerSize, save = true): void => {
    setSize(next);
    if (save) {
      saveDrawerSize(next);
    }
  };
  const drag = (event: PointerEvent<HTMLElement>, save: boolean): void => {
    const width = draggedWidth(event);
    if (width !== null) {
      update({ width, wide: false }, save);
    }
  };
  return {
    wide: size.wide,
    style: { "--drawer-width": `${size.width}px` } as CSSProperties,
    onToggleWide: () => update({ ...size, wide: !size.wide }),
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
        const width = keyedWidth(event.key, size.width);
        if (width !== null) {
          event.preventDefault();
          update({ width, wide: false });
        }
      },
      onDoubleClick: () => update(DEFAULT_SIZE),
      "aria-valuenow": size.width,
    },
  };
}
