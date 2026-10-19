import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { Bot } from "../../protocol/entities";
import { bot } from "../../test/fixtures";
import { teamDaemon } from "../../test/teamFixtures";
import BotHeader from "./BotHeader";
import BotPresenceSwitch from "./BotPresenceSwitch";
import { BotHeaderActions } from "./BotWakeActions";

const noop = (): void => {};

/** A bot's header and its Always on switch, on a service with the team on demand. */
function Frame(props: { readonly bot: Bot; readonly lead?: boolean }): ReactElement {
  const [client] = useState(teamDaemon);
  return (
    <div className="main" style={{ width: 720 }}>
      <BotHeader
        bot={props.bot}
        canControl
        infoPanelCollapsed
        onToggleInfoPanel={noop}
        actions={
          <BotHeaderActions client={client} bot={props.bot} connected canControl onToast={noop} />
        }
      />
      <div className="panel" style={{ maxWidth: 360 }}>
        <BotPresenceSwitch
          client={client}
          bot={props.bot}
          lead={props.lead === true}
          connected
          canControl
          onBotUpdated={noop}
          onToast={noop}
        />
      </div>
    </div>
  );
}

/** ◌ Asleep, not an error: Wake now, and the bot is on demand. */
export const Asleep: Story = () => (
  <Frame bot={bot({ name: "Tester", state: "asleep", presence: "on_demand" })} />
);

/** Woken while every slot is taken. */
export const WaitingForASlot: Story = () => (
  <Frame bot={bot({ name: "Developer", state: "queued", presence: "on_demand" })} />
);

/** An on-demand bot at work can be sent to sleep. */
export const AwakeOnDemand: Story = () => (
  <Frame bot={bot({ name: "Product", state: "working", presence: "on_demand" })} />
);

/** The lead is always on: the switch is on, off limits, and says why. */
export const Lead: Story = () => (
  <Frame bot={bot({ name: "Team Lead", state: "ready", presence: "always_on" })} lead />
);
