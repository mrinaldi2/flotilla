import { useEffect, useRef } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot } from "../../protocol/entities";
import { teamOn } from "../../protocol/team";
import type { BotTab } from "./BotTabs";

/** The tabs that show the bot's conversation: opening one wakes it. */
const CONVERSATION: ReadonlySet<BotTab> = new Set<BotTab>(["chat", "terminal"]);

/**
 * Opening an asleep bot's chat wakes it (0.23.0): the owner is about to talk
 * to it, so it starts ahead of board work. Sent once per bot until it is up
 * again; a refusal is left to the header's Wake now.
 */
export function useWakeOnOpen(
  client: DaemonApi,
  bot: Pick<Bot, "id" | "state">,
  tab: BotTab,
  connected: boolean,
  canControl: boolean,
): void {
  const woken = useRef<string | null>(null);
  const asleep = bot.state === "asleep";
  const wanted = asleep && CONVERSATION.has(tab) && connected && canControl && teamOn(client);
  useEffect(() => {
    if (!asleep) {
      woken.current = null;
      return;
    }
    if (!wanted || woken.current === bot.id) {
      return;
    }
    woken.current = bot.id;
    client.request({ type: "wake_bot", bot_id: bot.id }, "ok").catch(() => {
      woken.current = null;
    });
  }, [client, bot.id, asleep, wanted]);
}
