// "Discuss with a bot ▾" under a draft's description (0.19.0): the menu
// picks the bot, the lead first. Choosing one invites it to the draft and
// opens its chat beside the drawer.

import type { ReactElement } from "react";
import type { Bot } from "../../../protocol/entities";
import MenuButton from "../MenuButton";

interface DiscussButtonProps {
  readonly bots: readonly Bot[];
  readonly leadId: string | undefined;
  /** Bots already invited to the draft. */
  readonly invited: readonly string[];
  /** An invite is on its way: no other until it is answered. */
  readonly busy?: boolean;
  readonly onDiscuss: (bot: Bot) => void;
  readonly initiallyOpen?: boolean;
}

/** The project's bots, the lead first. */
function leadFirst(bots: readonly Bot[], leadId: string | undefined): Bot[] {
  const lead = bots.filter((b) => b.id === leadId);
  return [...lead, ...bots.filter((b) => b.id !== leadId)];
}

export default function DiscussButton(props: DiscussButtonProps): ReactElement | null {
  const bots = leadFirst(props.bots, props.leadId);
  const first = bots[0];
  if (first === undefined) {
    return null;
  }
  return (
    <div className="discuss-button">
      <MenuButton
        label={
          <>
            Discuss with a bot <span className="drawer-dim">· {first.name}</span>
          </>
        }
        ariaLabel="Discuss with a bot"
        menuLabel="Discuss the draft with"
        initiallyOpen={props.initiallyOpen}
        disabled={props.busy}
        entries={bots.map((bot) => ({
          disabled: props.busy,
          key: bot.id,
          label: bot.name,
          note: [
            bot.id === props.leadId ? "lead" : "",
            props.invited.includes(bot.id) ? "invited" : "",
          ]
            .filter(Boolean)
            .join(" · "),
          onChoose: () => props.onDiscuss(bot),
        }))}
      />
    </div>
  );
}
