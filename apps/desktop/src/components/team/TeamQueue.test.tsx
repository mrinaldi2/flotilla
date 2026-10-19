import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Bot } from "../../protocol/entities";
import * as fx from "../../test/fixtures";
import { TEAM_NOW, TEAM_STATUS } from "../../test/teamFixtures";
import TeamView from "./TeamView";

const BOTS: readonly Bot[] = [
  fx.bot({ id: "b1", name: "Team Lead", state: "working", presence: "always_on" }),
  fx.bot({ id: "b4", name: "Product", state: "ready", presence: "on_demand" }),
  fx.bot({ id: "b5", name: "Developer", state: "queued", presence: "on_demand" }),
  fx.bot({ id: "b6", name: "Tester", state: "asleep", presence: "on_demand" }),
];

function renderTeam(on = true) {
  const onWake = vi.fn<(botId: string) => void>();
  const onSleep = vi.fn<(botId: string) => void>();
  render(
    <TeamView
      bots={BOTS}
      row={null}
      threads={[]}
      unread={{}}
      failed={{}}
      activity={{}}
      leadBotId="b1"
      now={TEAM_NOW}
      canControl
      onOpenBot={vi.fn<(botId: string) => void>()}
      onCreateBot={vi.fn<() => void>()}
      onDeleteBot={vi.fn<(botId: string) => void>()}
      team={on ? TEAM_STATUS : null}
      onWake={on ? onWake : undefined}
      onSleep={on ? onSleep : undefined}
    />,
  );
  return { onWake, onSleep };
}

describe("the team on demand on the Team tab", () => {
  it("counts the running bots and lists the queue in order", () => {
    renderTeam();
    expect(screen.getByText("4 of 6 running · 2 waiting")).toBeInTheDocument();
    const rows = within(screen.getByRole("list", { name: "Waiting for a slot" })).getAllByRole(
      "listitem",
    );
    expect(rows.map((r) => r.textContent)).toEqual([
      "DeveloperUrgent bugCard T-31 · P0waiting 2 min",
      "TesterCard T-12 · P1waiting 1 h 13 min",
    ]);
  });

  it("shows Asleep and Waiting for a slot, with Wake now", async () => {
    const { onWake } = renderTeam();
    const tester = within(screen.getByRole("article", { name: "Tester" }));
    expect(tester.getByText("◌ Asleep")).toBeInTheDocument();
    await userEvent.click(tester.getByRole("button", { name: "Wake now" }));
    expect(onWake).toHaveBeenCalledWith("b6");
    const dev = within(screen.getByRole("article", { name: "Developer" }));
    expect(dev.getByText("◌ Waiting for a slot")).toBeInTheDocument();
    expect(dev.getByRole("button", { name: "Wake now" })).toBeInTheDocument();
    const lead = within(screen.getByRole("article", { name: "Team Lead" }));
    expect(lead.queryByRole("button", { name: "Wake now" })).toBeNull();
  });

  it("puts an on-demand bot that is up to sleep from its menu", async () => {
    const user = userEvent.setup();
    const { onSleep } = renderTeam();
    await user.click(screen.getByRole("button", { name: "More for Product" }));
    await user.click(screen.getByRole("menuitem", { name: "Sleep now" }));
    expect(onSleep).toHaveBeenCalledWith("b4");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "More for Team Lead" }));
    expect(screen.queryByRole("menuitem", { name: "Sleep now" })).toBeNull();
  });

  it("shows none of it before 0.23", () => {
    renderTeam(false);
    expect(screen.queryByText(/running/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Wake now" })).toBeNull();
  });
});
