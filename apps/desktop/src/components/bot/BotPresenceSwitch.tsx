import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, BotPresence, NotifyLevel } from "../../protocol/entities";
import { teamOn } from "../../protocol/team";
import { errText } from "../../util";

interface BotPresenceSwitchProps {
  readonly client: DaemonApi;
  readonly bot: Bot;
  /** It leads its project, and a lead is always on. */
  readonly lead: boolean;
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly onBotUpdated: (bot: Bot) => void;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

const HELP: Readonly<Record<BotPresence, string>> = {
  always_on: "Keeps running, and takes one of this computer's running slots all the time.",
  on_demand:
    "Sleeps after a few idle minutes and wakes when work arrives: a card, a task, a message, a routine or you opening its chat.",
};

/**
 * Always on, or on demand (0.23.0): an on-demand bot sleeps when it has
 * nothing to do and wakes with its conversation intact. The project's lead
 * is always on, so the switch is shown off for it.
 */
export default function BotPresenceSwitch(props: BotPresenceSwitchProps): ReactElement | null {
  const { client, bot, lead, connected, canControl, onBotUpdated, onToast } = props;
  const [saving, setSaving] = useState(false);
  if (!teamOn(client) || bot.temporary === true) {
    return null;
  }
  const presence: BotPresence = lead ? "always_on" : (bot.presence ?? "always_on");
  const linked = bot.peer != null;
  const save = async (next: BotPresence): Promise<void> => {
    setSaving(true);
    try {
      const reply = await client.request(
        { type: "set_bot_presence", bot_id: bot.id, presence: next },
        "bot",
      );
      onBotUpdated(reply.bot);
      onToast(
        "info",
        next === "always_on" ? "Always on" : "On demand",
        next === "always_on"
          ? `${reply.bot.name} keeps running from now on.`
          : `${reply.bot.name} sleeps once it is idle, and wakes when work arrives.`,
      );
    } catch (error) {
      onToast("error", "Couldn't change when the bot runs", errText(error));
    } finally {
      setSaving(false);
    }
  };
  const why = lead
    ? `${bot.name} leads the project, and a lead is always on.`
    : linked
      ? `${bot.name} runs on another computer; set this there.`
      : HELP[presence];
  return (
    <div className="settings-row bot-presence">
      <div className="settings-row-text">
        <label className="settings-row-label" htmlFor="bot-always-on">
          Always on
        </label>
        <div className="settings-row-help">{why}</div>
      </div>
      <label className="toggle" htmlFor="bot-always-on" aria-label="Always on">
        <input
          id="bot-always-on"
          type="checkbox"
          checked={presence === "always_on"}
          disabled={!connected || !canControl || saving || lead || linked}
          onChange={(event) => void save(event.target.checked ? "always_on" : "on_demand")}
        />
        <span className="toggle-track" />
      </label>
    </div>
  );
}
