import type { Story } from "@ladle/react";
import { decodeOverview, decodeOwnerThreads } from "../../protocol/home";
import { bot } from "../../test/fixtures";
import { HOME_NOW, overviewJson, ownerThreadsJson } from "../../test/homeFixtures";
import { TEAM_NOW, TEAM_STATUS } from "../../test/teamFixtures";
import TeamView from "./TeamView";

const row = decodeOverview(overviewJson()).rows.find((r) => r.projectId === "p1") ?? null;
const noop = (): void => {};

/** Bots as cards: state in words, what each does, what it last sent you. */
export const Team: Story = () => (
  <div className="main" style={{ height: 520 }}>
    <TeamView
      bots={[
        bot({ id: "b1", name: "Team Lead", state: "working" }),
        bot({ id: "b2", name: "Desktop Dev", state: "waiting_for_user", description: "" }),
        bot({ id: "b3", name: "Architect", state: "ready", description: "Reviews designs" }),
        bot({ id: "b4", name: "Tester Win", state: "crashed", description: "Tests on win-pc" }),
      ]}
      row={row}
      threads={decodeOwnerThreads(ownerThreadsJson()).threads}
      unread={{ b2: 1 }}
      failed={{}}
      activity={{}}
      leadBotId="b1"
      now={HOME_NOW}
      canControl
      onOpenBot={noop}
      onCreateBot={noop}
      onDeleteBot={noop}
    />
  </div>
);

/** The team on demand (0.23.0): the running count, its queue, Asleep and Waiting for a slot. */
export const OnDemand: Story = () => (
  <div className="main" style={{ height: 600 }}>
    <TeamView
      bots={[
        bot({ id: "b1", name: "Team Lead", state: "working", presence: "always_on" }),
        bot({ id: "b4", name: "Product", state: "ready", presence: "on_demand", description: "" }),
        bot({ id: "b5", name: "Developer", state: "queued", presence: "on_demand" }),
        bot({ id: "b6", name: "Tester", state: "asleep", presence: "on_demand" }),
      ]}
      row={null}
      threads={[]}
      unread={{}}
      failed={{}}
      activity={{}}
      leadBotId="b1"
      now={TEAM_NOW}
      canControl
      onOpenBot={noop}
      onCreateBot={noop}
      onDeleteBot={noop}
      team={TEAM_STATUS}
      onWake={noop}
      onSleep={noop}
    />
  </div>
);
