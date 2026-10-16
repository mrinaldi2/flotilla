// Client-side preferences: appearance and notification knobs that belong to
// this install, not the daemon. Persisted in localStorage, observable so open
// panes (e.g. terminals) restyle live when a setting changes.

import { useSyncExternalStore } from "react";
import { readStored, writeStored } from "./storage";

export interface Prefs {
  /** Terminal font size in px. */
  readonly terminalFontSize: number;
  /** Terminal font stack passed to xterm. */
  readonly terminalFontFamily: string;
  /** Mirror the unread total onto the dock icon. */
  readonly dockBadge: boolean;
  /** Global accelerator that shows or hides the window; empty means none. */
  readonly toggleWindowShortcut: string;
  /** Raise a native notification for an urgent or nearly-due decision. */
  readonly decisionNotifications: boolean;
  /** Which board activity drops a toast from the top (0.19.0). */
  readonly boardActivity: BoardActivityPrefs;
}

/** The kinds of board activity the owner can turn off one by one. */
export interface BoardActivityPrefs {
  /** A card moved: by a bot, the owner elsewhere, or automatically. */
  readonly moved: boolean;
  /** A card was assigned, or a bot picked one up. */
  readonly assigned: boolean;
  /** A move was refused, with why. */
  readonly refused: boolean;
  /** A bot started or finished a turn on a card. */
  readonly turns: boolean;
  /** A bot updated a draft. */
  readonly drafts: boolean;
  /** The board keeper asked a bot about its card, or raised a row for you. */
  readonly checks: boolean;
}

export type BoardActivityKind = keyof BoardActivityPrefs;

export const BOARD_ACTIVITY_KINDS: readonly BoardActivityKind[] = [
  "moved",
  "assigned",
  "refused",
  "turns",
  "drafts",
  "checks",
];

const DEFAULT_BOARD_ACTIVITY: BoardActivityPrefs = {
  moved: true,
  assigned: true,
  refused: true,
  turns: true,
  drafts: true,
  checks: true,
};

export const DEFAULT_PREFS: Prefs = {
  terminalFontSize: 13,
  terminalFontFamily: '"SF Mono", "Menlo", "Monaco", monospace',
  dockBadge: true,
  toggleWindowShortcut: "",
  decisionNotifications: true,
  boardActivity: DEFAULT_BOARD_ACTIVITY,
};

export const MIN_TERMINAL_FONT_SIZE = 9;
export const MAX_TERMINAL_FONT_SIZE = 24;

const PREFS_KEY = "prefs";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function fontSizeFrom(value: unknown): number {
  if (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= MIN_TERMINAL_FONT_SIZE &&
    value <= MAX_TERMINAL_FONT_SIZE
  ) {
    return value;
  }
  return DEFAULT_PREFS.terminalFontSize;
}

function boardActivityFrom(value: unknown): BoardActivityPrefs {
  if (!isRecord(value)) {
    return DEFAULT_BOARD_ACTIVITY;
  }
  const kind = (key: BoardActivityKind): boolean => {
    const stored = value[key];
    return typeof stored === "boolean" ? stored : DEFAULT_BOARD_ACTIVITY[key];
  };
  return {
    moved: kind("moved"),
    assigned: kind("assigned"),
    refused: kind("refused"),
    turns: kind("turns"),
    drafts: kind("drafts"),
    checks: kind("checks"),
  };
}

function sameBoardActivity(a: BoardActivityPrefs, b: BoardActivityPrefs): boolean {
  return BOARD_ACTIVITY_KINDS.every((kind) => a[kind] === b[kind]);
}

function prefsFrom(value: unknown): Prefs {
  if (!isRecord(value)) {
    return DEFAULT_PREFS;
  }
  return {
    terminalFontSize: fontSizeFrom(value["terminalFontSize"]),
    terminalFontFamily:
      typeof value["terminalFontFamily"] === "string" && value["terminalFontFamily"].length > 0
        ? value["terminalFontFamily"]
        : DEFAULT_PREFS.terminalFontFamily,
    dockBadge:
      typeof value["dockBadge"] === "boolean" ? value["dockBadge"] : DEFAULT_PREFS.dockBadge,
    toggleWindowShortcut:
      typeof value["toggleWindowShortcut"] === "string"
        ? value["toggleWindowShortcut"]
        : DEFAULT_PREFS.toggleWindowShortcut,
    decisionNotifications:
      typeof value["decisionNotifications"] === "boolean"
        ? value["decisionNotifications"]
        : DEFAULT_PREFS.decisionNotifications,
    boardActivity: boardActivityFrom(value["boardActivity"]),
  };
}

function loadPrefs(): Prefs {
  try {
    const raw = readStored(PREFS_KEY);
    if (raw === null) {
      return DEFAULT_PREFS;
    }
    return prefsFrom(JSON.parse(raw));
  } catch {
    return DEFAULT_PREFS;
  }
}

let current: Prefs = loadPrefs();
const listeners = new Set<() => void>();

export function getPrefs(): Prefs {
  return current;
}

export function subscribePrefs(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function updatePrefs(patch: Partial<Prefs>): void {
  const next = prefsFrom({ ...current, ...patch });
  if (
    next.terminalFontSize === current.terminalFontSize &&
    next.terminalFontFamily === current.terminalFontFamily &&
    next.dockBadge === current.dockBadge &&
    next.toggleWindowShortcut === current.toggleWindowShortcut &&
    next.decisionNotifications === current.decisionNotifications &&
    sameBoardActivity(next.boardActivity, current.boardActivity)
  ) {
    return;
  }
  current = next;
  try {
    writeStored(PREFS_KEY, JSON.stringify(current));
  } catch {
    // localStorage unavailable; prefs are session-only
  }
  for (const listener of listeners) {
    listener();
  }
}

/** Test hook: re-reads localStorage and drops listeners' stale snapshot. */
export function reloadPrefsForTest(): void {
  current = loadPrefs();
  for (const listener of listeners) {
    listener();
  }
}

/** The current prefs, re-rendering the caller whenever any of them change. */
export function usePrefs(): Prefs {
  return useSyncExternalStore(subscribePrefs, getPrefs);
}

function getDockBadge(): boolean {
  return current.dockBadge;
}

/** The dock-badge preference without rerendering for unrelated font changes. */
export function useDockBadgePref(): boolean {
  return useSyncExternalStore(subscribePrefs, getDockBadge);
}

function getDecisionNotifications(): boolean {
  return current.decisionNotifications;
}

/** Whether an urgent or nearly-due decision raises a native notification. */
export function useDecisionNotificationsPref(): boolean {
  return useSyncExternalStore(subscribePrefs, getDecisionNotifications);
}

function getBoardActivity(): BoardActivityPrefs {
  return current.boardActivity;
}

/** Which board activity toasts, without rerendering for unrelated prefs. */
export function useBoardActivityPrefs(): BoardActivityPrefs {
  return useSyncExternalStore(subscribePrefs, getBoardActivity);
}

/** Turn one kind of board activity toast on or off. */
export function setBoardActivity(kind: BoardActivityKind, on: boolean): void {
  updatePrefs({ boardActivity: { ...current.boardActivity, [kind]: on } });
}
