import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, NotifyLevel } from "../../protocol/entities";
import { canSleep, resting, teamOn } from "../../protocol/team";
import { wakeOrSleep } from "../team/teamActions";
import BotSessionActions from "./BotSessionActions";

interface BotWakeActionsProps {
  readonly client: DaemonApi;
  readonly bot: Bot;
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

/**
 * The team on demand in the bot's header (0.23.0): "Wake now" for a bot that
 * is asleep or waiting for a slot, "Sleep now" for an on-demand bot that is
 * up. An always-on bot has neither.
 */
export default function BotWakeActions(props: BotWakeActionsProps): ReactElement | null {
  const { client, bot, connected, canControl, onToast } = props;
  const [busy, setBusy] = useState(false);
  if (!canControl || !teamOn(client)) {
    return null;
  }
  const action = resting(bot) ? "wake" : canSleep(bot) ? "sleep" : null;
  if (action === null) {
    return null;
  }
  const run = async (): Promise<void> => {
    setBusy(true);
    await wakeOrSleep(client, bot, action, onToast);
    setBusy(false);
  };
  return (
    <button
      type="button"
      className={action === "wake" ? "btn btn-small btn-primary" : "btn btn-small"}
      disabled={!connected || busy}
      title={
        action === "wake"
          ? "Start it now, ahead of board work"
          : "Stop its session once the current turn ends; work for it wakes it"
      }
      onClick={() => void run()}
    >
      {action === "wake" ? "Wake now" : "Sleep now"}
    </button>
  );
}

/** The header's session actions: Wake now or Sleep now, then Restart and Clear. */
export function BotHeaderActions(props: BotWakeActionsProps): ReactElement {
  return (
    <>
      <BotWakeActions {...props} />
      <BotSessionActions {...props} />
    </>
  );
}
