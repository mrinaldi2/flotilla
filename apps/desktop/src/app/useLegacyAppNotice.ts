import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { readStored, writeStored } from "../storage";
import { isTauri } from "../tauri";
import type { AddToast } from "./useToasts";

const SHOWN_KEY = "legacy-app-notice";
const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * What the owner reads while The Hermes is still installed beside Flotilla.
 * macOS keys permission grants by bundle id, and Flotilla has its own, so it
 * asks for them again.
 */
export function legacyAppMessage(path: string): string {
  return (
    `Flotilla replaces The Hermes, and has taken over its settings and its service. ` +
    `You can move ${path} to the Trash. ` +
    "macOS asks again for microphone, speech recognition and accessibility access, " +
    "because Flotilla is a new app to it."
  );
}

/** Whether the notice was shown less than a day before `now`. */
function shownRecently(now: number): boolean {
  try {
    const stored = readStored(SHOWN_KEY);
    const shown = stored === null ? Number.NaN : Number(stored);
    return Number.isFinite(shown) && now - shown < DAY_MS;
  } catch {
    return false;
  }
}

/**
 * Once a day, while the app this one replaced is still installed, tells the
 * owner it can go. Never deletes it.
 */
export function useLegacyAppNotice(addToast: AddToast, now: () => number = Date.now): void {
  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let live = true;
    void (async (): Promise<void> => {
      let path: unknown;
      try {
        path = await invoke("legacy_app_path");
      } catch {
        return;
      }
      if (!live || typeof path !== "string" || shownRecently(now())) {
        return;
      }
      try {
        writeStored(SHOWN_KEY, String(now()));
      } catch {
        // Unavailable storage only means the notice may come again.
      }
      addToast("info", "The Hermes is still installed", legacyAppMessage(path), {
        sticky: true,
      });
    })();
    return (): void => {
      live = false;
    };
  }, [addToast, now]);
}
