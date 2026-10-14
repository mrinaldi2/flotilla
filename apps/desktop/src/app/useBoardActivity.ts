import { useCallback, useEffect, useRef, useState } from "react";
import type { DaemonApi } from "../protocol/api";
import type { Bot } from "../protocol/entities";
import { decodeBoardActivity } from "../protocol/home";
import type { BoardActivityPrefs } from "../prefs";
import {
  EMPTY_STACK,
  addNotice,
  allowed,
  dismissToast,
  expireStack,
  noticeFromActivity,
  noticeFromBotState,
} from "./boardActivity";
import type { ActivityNotice, ActivityStack, ActivityToast } from "./boardActivity";
import type { Selection } from "./selection";
import { useLatestRef } from "./useLatestRef";

/** How often expired toasts are swept. */
const SWEEP_MS = 1000;

export interface BoardActivityDeps {
  readonly client: DaemonApi;
  readonly bots: readonly Bot[];
  readonly prefs: BoardActivityPrefs;
  readonly select: (selection: Selection) => void;
  /** The clock; tests pass their own. */
  readonly now?: () => number;
}

export interface BoardActivityApi {
  readonly toasts: readonly ActivityToast[];
  /** Opens the toast's card in its drawer on the Board, and drops the toast. */
  readonly open: (toast: ActivityToast) => void;
  readonly dismiss: (id: number) => void;
  /** Holds every toast up while the owner reads or tabs through them. */
  readonly hold: (held: boolean) => void;
}

/**
 * Turns `board_activity` and `bot_state` pushes into the app-wide toasts that
 * drop from the top: grouped per actor and card, at most three, each a way
 * to the card's drawer. The kinds the owner turned off in Settings say nothing.
 */
export function useBoardActivity(deps: BoardActivityDeps): BoardActivityApi {
  const ref = useLatestRef(deps);
  const [stack, setStack] = useState<ActivityStack>(EMPTY_STACK);
  const held = useRef(false);
  const clock = useCallback((): number => (ref.current.now ?? Date.now)(), [ref]);

  useEffect(() => {
    const show = (notice: ActivityNotice | null): void => {
      if (notice === null || !allowed(notice, ref.current.prefs)) {
        return;
      }
      const at = clock();
      setStack((prev) => addNotice(prev, notice, at));
    };
    const botName = (botId: string): string | undefined =>
      ref.current.bots.find((bot) => bot.id === botId)?.name;
    const turns = new Map<string, string>();
    const offActivity = ref.current.client.on("board_activity", (push) => {
      show(noticeFromActivity(decodeBoardActivity(push.activity), botName));
    });
    const offState = ref.current.client.on("bot_state", (push) => {
      const bot = ref.current.bots.find((b) => b.id === push.bot_id);
      const turnBot = bot === undefined ? undefined : { name: bot.name, projectId: bot.project_id };
      show(noticeFromBotState(push, turns, turnBot));
    });
    const sweep = setInterval(() => {
      if (!held.current) {
        const at = clock();
        setStack((prev) => expireStack(prev, at));
      }
    }, SWEEP_MS);
    return () => {
      offActivity();
      offState();
      clearInterval(sweep);
    };
  }, [ref, clock]);

  const dismiss = useCallback((id: number): void => {
    setStack((prev) => dismissToast(prev, id));
  }, []);

  const open = useCallback(
    (toast: ActivityToast): void => {
      ref.current.select({
        kind: "project",
        projectId: toast.projectId,
        tab: "board",
        item: toast.itemId,
      });
      dismiss(toast.id);
    },
    [ref, dismiss],
  );

  const hold = useCallback(
    (on: boolean): void => {
      held.current = on;
      if (!on) {
        // What expired while held goes once let go, not all at once later.
        const at = clock();
        setStack((prev) => ({
          ...prev,
          toasts: prev.toasts.map((t) => ({ ...t, updatedAt: at })),
        }));
      }
    },
    [clock],
  );

  return { toasts: stack.toasts, open, dismiss, hold };
}
