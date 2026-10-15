import { useDockBadgePref } from "../prefs";
import { useDockBadge } from "./useDockBadge";
import type { AddToast } from "./useToasts";
import { useWindowShortcut } from "./useWindowShortcut";
import { useLegacyAppNotice } from "./useLegacyAppNotice";

/** What the desktop shell mirrors from app state: the dock badge, the global shortcut, and the notice about the app this one replaced. */
export function useDesktopShell(
  unreadBots: Readonly<Record<string, number>>,
  pendingDecisions: number,
  addToast: AddToast,
): void {
  useDockBadge(unreadBots, pendingDecisions, useDockBadgePref());
  useWindowShortcut(addToast);
  useLegacyAppNotice(addToast);
}
