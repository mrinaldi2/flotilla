// The team on demand (0.23.0): this computer's running count and queue,
// read on connect and kept by `team_updated`, and the owner waking a bot or
// sending it to sleep. A daemon without `team_on_demand_v1` has neither.

import { useCallback, useEffect, useState } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, NotifyLevel, TeamStatus } from "../../protocol/entities";
import { teamOn } from "../../protocol/team";
import { errText } from "../../util";

/** The running count and the queue for a slot; null until read, or without the capability. */
export function useTeamStatus(client: DaemonApi, connected: boolean): TeamStatus | null {
  const supported = teamOn(client);
  const [team, setTeam] = useState<TeamStatus | null>(null);
  const load = useCallback(async (): Promise<void> => {
    if (!supported) {
      return;
    }
    try {
      const reply = await client.request({ type: "team_status" }, "team");
      setTeam(reply.team);
    } catch {
      // The counter is a convenience; the next `team_updated` brings it.
    }
  }, [client, supported]);
  useLoadOnConnect(connected, load);
  useEffect(() => {
    if (!supported) {
      return undefined;
    }
    return client.on("team_updated", (push) => {
      setTeam(push.team);
    });
  }, [client, supported]);
  return supported ? team : null;
}

type Toast = (level: NotifyLevel, title: string, body: string) => void;

/** Wakes `bot` ahead of board work, or sends it to sleep once its turn ends. */
export async function wakeOrSleep(
  client: DaemonApi,
  bot: Pick<Bot, "id" | "name">,
  action: "wake" | "sleep",
  onToast: Toast,
): Promise<void> {
  const wake = action === "wake";
  try {
    await client.request({ type: wake ? "wake_bot" : "sleep_bot", bot_id: bot.id }, "ok");
    onToast(
      "info",
      wake ? `Waking ${bot.name}` : `${bot.name} will sleep`,
      wake
        ? `${bot.name} starts now, or as soon as a running slot frees.`
        : `${bot.name} goes to sleep once its current turn ends. Work for it wakes it.`,
    );
  } catch (error) {
    onToast(
      "error",
      wake ? `Couldn't wake ${bot.name}` : `Couldn't put ${bot.name} to sleep`,
      errText(error),
    );
  }
}
